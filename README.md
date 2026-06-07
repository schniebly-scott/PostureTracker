# Posture Tracker

A local desktop application for tracking your posture while working on a computer with a webcam. The goals are accuracy, low-resource usage, and intuitive UI.

<img width="1206" height="963" alt="Screenshot From 2026-06-03 17-14-09" src="https://github.com/user-attachments/assets/5ecfe4c7-3b25-446c-9271-73dec11348a3" />

## Built With
- Made entirely in Rust, mainly using the ort library for inference and iced for the UI.
- The pose detection is the YOLO pose model by Ultralytics (version may change in future updates).

### AI Usage Disclaimer
Claude Code was used in the development of this application. It made it much easier to create a good looking UI without spending too much time reading documentation. Future updates will be mostly human written code and core functionalities of the application are mostly human written.

## Process
**UI Rough Draft in LibreOffice**
<img width="300" height="212" alt="image" src="https://github.com/user-attachments/assets/86ed8294-d495-41f4-a7cc-07b02c6bff5c" />

**Rust Library Test Project**
Link: [rust-webcam-model-bench](https://github.com/schniebly-scott/rust-webcam-model-bench)
Used this simple project to make sure that the libraries such as ort and ccap-rs could be integrated with Iced. Also, it allowed me to test multiple models on my laptop for efficency in a similar environment.

**POC Project**
Link: [PostureTracker_PoC](https://github.com/schniebly-scott/PostureTracker_PoC)
I built this to see if it would be feasible to capture posture data using RF-DETR and ViTPose computer vision models through a laptop webcamera. Another goal of the experiment was using a low frame rate of images passed into the model allowing it to run on an edge device without taking up too many resources.
