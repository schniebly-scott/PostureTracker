use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate};

const HISTORY_SECS: f64 = 120.0;

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

pub struct MetricsStore {
    pub angle_history: VecDeque<AngleSample>,

    breaks_today: u32,
    bad_posture_secs_today: f64,
    tracked_secs_today: f64,

    tracking_since: Option<Instant>,
    bad_posture_since: Option<Instant>,
    streak_since: Option<Instant>,
    last_broke_at: Option<Instant>,

    today: NaiveDate,
    keep_days: i64,
    data_dir: PathBuf,
    log_file: Option<File>,
}

impl MetricsStore {
    pub fn new(keep_days: i64) -> Self {
        let data_dir = dirs::data_dir()
            .map(|d| d.join("posturetracker"))
            .unwrap_or_else(|| PathBuf::from(".posturetracker"));

        let _ = fs::create_dir_all(&data_dir);

        let today = Local::now().date_naive();

        let mut store = Self {
            angle_history: VecDeque::new(),
            breaks_today: 0,
            bad_posture_secs_today: 0.0,
            tracked_secs_today: 0.0,
            tracking_since: None,
            bad_posture_since: None,
            streak_since: None,
            last_broke_at: None,
            today,
            keep_days,
            data_dir,
            log_file: None,
        };

        store.load_today();
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
        self.append_log(LogEvent::Start);
    }

    pub fn stop_tracking(&mut self) {
        if let Some(since) = self.tracking_since.take() {
            self.tracked_secs_today += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.bad_posture_since.take() {
            self.bad_posture_secs_today += since.elapsed().as_secs_f64();
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

        let was_bad = self.bad_posture_since.is_some();

        if is_bad_posture && !was_bad {
            self.breaks_today += 1;
            self.bad_posture_since = Some(now);
            self.last_broke_at = Some(now);
            self.streak_since = None;
            self.append_log(LogEvent::GoodToBad);
        } else if !is_bad_posture && was_bad {
            if let Some(since) = self.bad_posture_since.take() {
                self.bad_posture_secs_today += since.elapsed().as_secs_f64();
            }
            self.streak_since = Some(now);
            self.append_log(LogEvent::BadToGood);
        }
    }

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

    fn rollover(&mut self, new_date: NaiveDate) {
        if let Some(since) = self.tracking_since.take() {
            self.tracked_secs_today += since.elapsed().as_secs_f64();
        }
        if let Some(since) = self.bad_posture_since.take() {
            self.bad_posture_secs_today += since.elapsed().as_secs_f64();
        }
        self.append_log(LogEvent::Stop);

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
