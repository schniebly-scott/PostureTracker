# Posture Tracker

A local desktop application for tracking your posture while working on a computer with a webcam. The goals are accuracy, low-resource usage, and intuitive UI.

<img width="1206" height="963" alt="Screenshot From 2026-06-03 17-14-09" src="https://github.com/user-attachments/assets/5ecfe4c7-3b25-446c-9271-73dec11348a3" />

## Built With
- Made entirely in Rust, mainly using the ort library for inference and iced for the UI.
- The pose detection is the YOLO pose model by Ultralytics (version may change in future updates).

### AI Usage Disclaimer
Claude Code was used in the development of this application. Architecture decisions and code reviews are strictly non-AI.

## Installation

### First launch on Windows (SmartScreen)
The Windows release is currently distributed as an unsigned `.exe`, so on first launch
Microsoft Defender SmartScreen may show a blue **"Windows protected your PC"** dialog
warning about an unrecognized app from an unknown publisher. This is expected for any
new, unsigned indie binary — it has not yet accumulated SmartScreen reputation — and does
**not** mean the download is unsafe.

To run it anyway:
1. On the SmartScreen dialog, click **More info**, then **Run anyway**.
2. Alternatively, before launching: right-click the downloaded `.exe` → **Properties**,
   tick **Unblock** at the bottom of the **General** tab, then **OK**. This removes the
   Mark-of-the-Web so SmartScreen won't prompt.

We plan to ship a code-signed build in the future, which will remove this prompt.

### First launch on macOS ("app is damaged")
The macOS release is not yet signed with an Apple Developer ID or notarized, so after you
download and unzip it, macOS Gatekeeper marks it as quarantined and shows
**"Posture Tracker.app is damaged and can't be opened"** (or an "unidentified developer"
warning). This is expected for an un-notarized indie app and does **not** mean the download
is unsafe or actually corrupted.

To run it, remove the quarantine attribute after moving the app to **Applications** (or
wherever you keep it):

```sh
xattr -cr "/Applications/Posture Tracker.app"
```

Then open it normally. (For the "damaged" variant, right-click → **Open** usually does not
work — the `xattr` command above is the reliable fix.)

We plan to ship a signed and notarized build in the future, which will remove this step.

## Process
**UI Rough Draft in LibreOffice**

<img width="300" height="212" alt="image" src="https://github.com/user-attachments/assets/86ed8294-d495-41f4-a7cc-07b02c6bff5c" />

**Rust Library Test Project**
Link: [rust-webcam-model-bench](https://github.com/schniebly-scott/rust-webcam-model-bench)
Used this simple project to make sure that the libraries such as ort and ccap-rs could be integrated with Iced. Also, it allowed me to test multiple models on my laptop for efficency in a similar environment.

**POC Project**
Link: [PostureTracker_PoC](https://github.com/schniebly-scott/PostureTracker_PoC)
I built this to see if it would be feasible to capture posture data using RF-DETR and ViTPose computer vision models through a laptop webcamera. Another goal of the experiment was using a low frame rate of images passed into the model allowing it to run on an edge device without taking up too many resources.
