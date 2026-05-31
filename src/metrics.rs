use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

const HISTORY_SECS: f64 = 120.0;
const ALL_TIME_FILE: &str = "all_time.toml";

#[derive(Clone, Debug)]
pub struct AngleSample {
    pub captured_at: Instant,
    pub angle_deg: Option<f32>,
    pub is_bad_posture: bool,
}

enum LogEvent {
    Start,
    Stop,
    GoodToBad,
    BadToGood,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct AllTimeTotals {
    #[serde(default)]
    breaks: u32,
    #[serde(default)]
    bad_posture_secs: f64,
    #[serde(default)]
    tracked_secs: f64,
    #[serde(default)]
    days_tracked: u32,
    #[serde(default)]
    last_counted_date: Option<NaiveDate>,
}

pub struct MetricsStore {
    pub angle_history: VecDeque<AngleSample>,

    breaks_today: u32,
    bad_posture_secs_today: f64,
    tracked_secs_today: f64,

    breaks_session: u32,
    bad_posture_secs_session: f64,
    tracked_secs_session: f64,
    session_started_at: Option<Instant>,
    bad_posture_session_since: Option<Instant>,

    tracking_since: Option<Instant>,
    bad_posture_since: Option<Instant>,
    streak_since: Option<Instant>,
    last_broke_at: Option<Instant>,

    today: NaiveDate,
    keep_days: i64,
    data_dir: PathBuf,
    log_file: Option<File>,

    all_time: AllTimeTotals,
    all_time_path: PathBuf,
}

impl MetricsStore {
    pub fn new(keep_days: i64) -> Self {
        let data_dir = dirs::data_dir()
            .map(|d| d.join("posturetracker"))
            .unwrap_or_else(|| PathBuf::from(".posturetracker"));

        let _ = fs::create_dir_all(&data_dir);

        let today = Local::now().date_naive();
        let all_time_path = data_dir.join(ALL_TIME_FILE);

        let mut store = Self {
            angle_history: VecDeque::new(),
            breaks_today: 0,
            bad_posture_secs_today: 0.0,
            tracked_secs_today: 0.0,
            breaks_session: 0,
            bad_posture_secs_session: 0.0,
            tracked_secs_session: 0.0,
            session_started_at: None,
            bad_posture_session_since: None,
            tracking_since: None,
            bad_posture_since: None,
            streak_since: None,
            last_broke_at: None,
            today,
            keep_days,
            data_dir,
            log_file: None,
            all_time: AllTimeTotals::default(),
            all_time_path,
        };

        store.load_today();
        store.load_all_time();
        store.bootstrap_all_time_from_logs();
        store.prune_old_logs();
        store.open_log_file();
        store
    }

    pub fn start_tracking(&mut self) {
        if self.tracking_since.is_some() {
            return;
        }
        let now = Instant::now();
        self.tracking_since = Some(now);
        self.streak_since = Some(now);
        self.session_started_at = Some(now);
        self.breaks_session = 0;
        self.bad_posture_secs_session = 0.0;
        self.tracked_secs_session = 0.0;
        self.bad_posture_session_since = None;
        self.append_log(LogEvent::Start);
    }

    pub fn stop_tracking(&mut self) {
        if let Some(since) = self.tracking_since.take() {
            self.tracked_secs_today += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.session_started_at.take() {
            self.tracked_secs_session += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.bad_posture_since.take() {
            self.bad_posture_secs_today += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.bad_posture_session_since.take() {
            self.bad_posture_secs_session += since.elapsed().as_secs_f64();
        }
        self.streak_since = None;
        self.append_log(LogEvent::Stop);
    }

    pub fn ingest(&mut self, angle_deg: Option<f32>, is_bad_posture: bool) {
        let now = Instant::now();

        let today = Local::now().date_naive();
        if today != self.today {
            self.rollover(today);
        }

        // Always record samples for the angle chart so testing/calibrating remains visible.
        self.angle_history.push_back(AngleSample {
            captured_at: now,
            angle_deg,
            is_bad_posture,
        });
        while self
            .angle_history
            .front()
            .is_some_and(|s| s.captured_at.elapsed().as_secs_f64() > HISTORY_SECS)
        {
            self.angle_history.pop_front();
        }

        // Metric counters only advance during an active tracking session
        // (i.e. background mode). Testing and calibration leave the counters frozen.
        if !self.is_session_active() {
            return;
        }

        let was_bad = self.bad_posture_since.is_some();

        if is_bad_posture && !was_bad {
            self.breaks_today += 1;
            self.breaks_session += 1;
            self.bad_posture_since = Some(now);
            self.bad_posture_session_since = Some(now);
            self.last_broke_at = Some(now);
            self.streak_since = None;
            self.append_log(LogEvent::GoodToBad);
        } else if !is_bad_posture && was_bad {
            if let Some(since) = self.bad_posture_since.take() {
                self.bad_posture_secs_today += since.elapsed().as_secs_f64();
            }
            if let Some(since) = self.bad_posture_session_since.take() {
                self.bad_posture_secs_session += since.elapsed().as_secs_f64();
            }
            self.streak_since = Some(now);
            self.append_log(LogEvent::BadToGood);
        }
    }

    // --- Daily getters ---

    pub fn breaks_today(&self) -> u32 {
        self.breaks_today
    }

    pub fn bad_posture_duration_today(&self) -> Duration {
        let extra = self
            .bad_posture_since
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        Duration::from_secs_f64(self.bad_posture_secs_today + extra)
    }

    pub fn tracked_duration_today(&self) -> Duration {
        let extra = self
            .tracking_since
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        Duration::from_secs_f64(self.tracked_secs_today + extra)
    }

    pub fn time_since_last_break(&self) -> Option<Duration> {
        self.last_broke_at.map(|t| t.elapsed())
    }

    pub fn good_posture_streak(&self) -> Option<Duration> {
        self.streak_since.map(|t| t.elapsed())
    }

    pub fn posture_quality_today(&self) -> f32 {
        let tracked = self.tracked_duration_today().as_secs_f64();
        if tracked <= 0.0 {
            return 1.0;
        }
        let bad = self.bad_posture_duration_today().as_secs_f64();
        (1.0 - (bad / tracked).clamp(0.0, 1.0)) as f32
    }

    // --- Session getters ---

    pub fn breaks_session(&self) -> u32 {
        self.breaks_session
    }

    pub fn bad_posture_duration_session(&self) -> Duration {
        let extra = self
            .bad_posture_session_since
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        Duration::from_secs_f64(self.bad_posture_secs_session + extra)
    }

    pub fn tracked_duration_session(&self) -> Duration {
        let extra = self
            .session_started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        Duration::from_secs_f64(self.tracked_secs_session + extra)
    }

    pub fn posture_quality_session(&self) -> f32 {
        let tracked = self.tracked_duration_session().as_secs_f64();
        if tracked <= 0.0 {
            return 1.0;
        }
        let bad = self.bad_posture_duration_session().as_secs_f64();
        (1.0 - (bad / tracked).clamp(0.0, 1.0)) as f32
    }

    pub fn is_session_active(&self) -> bool {
        self.session_started_at.is_some()
    }

    // --- All-time getters ---

    pub fn all_time_breaks(&self) -> u32 {
        // Lifetime totals + today's running count (since today isn't yet rolled into all-time).
        self.all_time.breaks + self.breaks_today
    }

    pub fn all_time_bad_posture_duration(&self) -> Duration {
        let today_bad = self.bad_posture_duration_today().as_secs_f64();
        Duration::from_secs_f64(self.all_time.bad_posture_secs + today_bad)
    }

    pub fn all_time_tracked_duration(&self) -> Duration {
        let today_tracked = self.tracked_duration_today().as_secs_f64();
        Duration::from_secs_f64(self.all_time.tracked_secs + today_tracked)
    }

    pub fn all_time_days_tracked(&self) -> u32 {
        let today_counts = if self.tracked_duration_today().as_secs_f64() > 0.0 {
            1
        } else {
            0
        };
        self.all_time.days_tracked + today_counts
    }

    // --- Reset methods ---

    pub fn reset_session(&mut self) {
        let now = Instant::now();
        self.breaks_session = 0;
        self.bad_posture_secs_session = 0.0;
        self.tracked_secs_session = 0.0;
        if self.tracking_since.is_some() {
            self.session_started_at = Some(now);
            if self.bad_posture_since.is_some() {
                self.bad_posture_session_since = Some(now);
            } else {
                self.bad_posture_session_since = None;
            }
        } else {
            self.session_started_at = None;
            self.bad_posture_session_since = None;
        }
    }

    pub fn reset_today(&mut self) {
        let now = Instant::now();
        self.breaks_today = 0;
        self.bad_posture_secs_today = 0.0;
        self.tracked_secs_today = 0.0;
        self.last_broke_at = None;

        if self.tracking_since.is_some() {
            self.tracking_since = Some(now);
            self.streak_since = if self.bad_posture_since.is_some() {
                None
            } else {
                Some(now)
            };
            if self.bad_posture_since.is_some() {
                self.bad_posture_since = Some(now);
            }
        } else {
            self.tracking_since = None;
            self.streak_since = None;
            self.bad_posture_since = None;
        }

        // Truncate today's log; if tracking, re-emit Start so the file reflects current state.
        let was_tracking = self.tracking_since.is_some();
        let was_bad = self.bad_posture_since.is_some();
        self.log_file = None;
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.log_path())
        {
            self.log_file = Some(file);
        }
        if was_tracking {
            self.append_log(LogEvent::Start);
            if was_bad {
                self.append_log(LogEvent::GoodToBad);
            }
        }

        self.reset_session();
    }

    pub fn reset_all_time(&mut self) {
        self.all_time = AllTimeTotals::default();
        let _ = fs::remove_file(&self.all_time_path);

        // Remove every .log file except today's, then reset today's.
        if let Ok(entries) = fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if let Some(stem) = s.strip_suffix(".log") {
                    if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                        if date != self.today {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        self.reset_today();
        self.save_all_time();
    }

    // --- Internal: rollover, persistence ---

    fn rollover(&mut self, new_date: NaiveDate) {
        if let Some(since) = self.tracking_since.take() {
            self.tracked_secs_today += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.bad_posture_since.take() {
            self.bad_posture_secs_today += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.session_started_at.take() {
            self.tracked_secs_session += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.bad_posture_session_since.take() {
            self.bad_posture_secs_session += since.elapsed().as_secs_f64();
        }
        self.append_log(LogEvent::Stop);

        // Roll the day we're closing out into all-time totals.
        if self.all_time.last_counted_date != Some(self.today) {
            self.all_time.breaks += self.breaks_today;
            self.all_time.bad_posture_secs += self.bad_posture_secs_today;
            self.all_time.tracked_secs += self.tracked_secs_today;
            if self.tracked_secs_today > 0.0 {
                self.all_time.days_tracked += 1;
            }
            self.all_time.last_counted_date = Some(self.today);
            self.save_all_time();
        }

        self.breaks_today = 0;
        self.bad_posture_secs_today = 0.0;
        self.tracked_secs_today = 0.0;
        self.streak_since = None;
        self.last_broke_at = None;
        self.today = new_date;

        self.log_file = None;
        self.open_log_file();
        self.prune_old_logs();
    }

    fn load_today(&mut self) {
        let path = self.log_path();
        let Ok(file) = File::open(&path) else {
            return;
        };

        let mut last_start_ms: Option<i64> = None;
        let mut last_bad_ms: Option<i64> = None;

        for line in BufReader::new(file).lines().flatten() {
            let mut parts = line.splitn(2, ',');
            let Some(ts_str) = parts.next() else {
                continue;
            };
            let Some(event) = parts.next() else {
                continue;
            };
            let Ok(ts) = ts_str.parse::<i64>() else {
                continue;
            };

            match event {
                "Start" => {
                    last_start_ms = Some(ts);
                }
                "Stop" => {
                    if let Some(start) = last_start_ms.take() {
                        self.tracked_secs_today += (ts - start).max(0) as f64 / 1000.0;
                    }
                    if let Some(bad) = last_bad_ms.take() {
                        self.bad_posture_secs_today += (ts - bad).max(0) as f64 / 1000.0;
                    }
                }
                "GoodToBad" => {
                    self.breaks_today += 1;
                    last_bad_ms = Some(ts);
                }
                "BadToGood" => {
                    if let Some(bad) = last_bad_ms.take() {
                        self.bad_posture_secs_today += (ts - bad).max(0) as f64 / 1000.0;
                    }
                }
                _ => {}
            }
        }
    }

    fn load_all_time(&mut self) {
        let Ok(contents) = fs::read_to_string(&self.all_time_path) else {
            return;
        };
        if let Ok(parsed) = toml::from_str::<AllTimeTotals>(&contents) {
            self.all_time = parsed;
        }
    }

    fn save_all_time(&self) {
        if let Ok(toml_str) = toml::to_string_pretty(&self.all_time) {
            let _ = fs::write(&self.all_time_path, toml_str);
        }
    }

    /// Aggregates any past-day logs not yet counted in `all_time` and adds them in.
    /// Idempotent: dates are filtered by `last_counted_date`.
    fn bootstrap_all_time_from_logs(&mut self) {
        let Ok(entries) = fs::read_dir(&self.data_dir) else {
            return;
        };

        let mut new_days: Vec<(NaiveDate, PathBuf)> = Vec::new();
        let last_counted = self.all_time.last_counted_date;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            let Some(stem) = s.strip_suffix(".log") else {
                continue;
            };
            let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") else {
                continue;
            };
            if date >= self.today {
                continue;
            }
            if last_counted.map_or(false, |lc| date <= lc) {
                continue;
            }
            new_days.push((date, entry.path()));
        }

        if new_days.is_empty() {
            return;
        }

        new_days.sort_by_key(|(d, _)| *d);

        for (date, path) in &new_days {
            let (breaks, bad_secs, tracked_secs) = parse_log_totals(path);
            self.all_time.breaks += breaks;
            self.all_time.bad_posture_secs += bad_secs;
            self.all_time.tracked_secs += tracked_secs;
            if tracked_secs > 0.0 {
                self.all_time.days_tracked += 1;
            }
            self.all_time.last_counted_date = Some(*date);
        }

        self.save_all_time();
    }

    fn prune_old_logs(&self) {
        let cutoff = self.today - chrono::Duration::days(self.keep_days);
        let Ok(entries) = fs::read_dir(&self.data_dir) else {
            return;
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if let Some(stem) = s.strip_suffix(".log") {
                if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                    if date < cutoff {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    fn open_log_file(&mut self) {
        self.log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .ok();
    }

    fn log_path(&self) -> PathBuf {
        self.data_dir
            .join(format!("{}.log", self.today.format("%Y-%m-%d")))
    }

    fn append_log(&mut self, event: LogEvent) {
        let ts = Local::now().timestamp_millis();
        let label = match event {
            LogEvent::Start => "Start",
            LogEvent::Stop => "Stop",
            LogEvent::GoodToBad => "GoodToBad",
            LogEvent::BadToGood => "BadToGood",
        };
        if let Some(ref mut f) = self.log_file {
            let _ = writeln!(f, "{},{}", ts, label);
        }
    }
}

fn parse_log_totals(path: &Path) -> (u32, f64, f64) {
    let Ok(file) = File::open(path) else {
        return (0, 0.0, 0.0);
    };

    let mut breaks: u32 = 0;
    let mut bad_secs: f64 = 0.0;
    let mut tracked_secs: f64 = 0.0;
    let mut last_start_ms: Option<i64> = None;
    let mut last_bad_ms: Option<i64> = None;

    for line in BufReader::new(file).lines().flatten() {
        let mut parts = line.splitn(2, ',');
        let Some(ts_str) = parts.next() else {
            continue;
        };
        let Some(event) = parts.next() else {
            continue;
        };
        let Ok(ts) = ts_str.parse::<i64>() else {
            continue;
        };

        match event {
            "Start" => {
                last_start_ms = Some(ts);
            }
            "Stop" => {
                if let Some(start) = last_start_ms.take() {
                    tracked_secs += (ts - start).max(0) as f64 / 1000.0;
                }
                if let Some(bad) = last_bad_ms.take() {
                    bad_secs += (ts - bad).max(0) as f64 / 1000.0;
                }
            }
            "GoodToBad" => {
                breaks += 1;
                last_bad_ms = Some(ts);
            }
            "BadToGood" => {
                if let Some(bad) = last_bad_ms.take() {
                    bad_secs += (ts - bad).max(0) as f64 / 1000.0;
                }
            }
            _ => {}
        }
    }

    (breaks, bad_secs, tracked_secs)
}
