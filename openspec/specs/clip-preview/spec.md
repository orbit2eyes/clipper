## Purpose

TBD — see archived change for original Why/Capabilities context.

## Requirements

### Requirement: Render source frame in preview area

The system SHALL display a single decoded frame of the source video in a preview area of the main window.

The system SHALL show the frame that corresponds to the current playhead position; if the playhead is at `t` seconds, the displayed frame MUST be the frame whose presentation timestamp is closest to `t`.

The system SHALL request the frame from ffmpeg as a single image and SHALL NOT decode the entire video into memory.

#### Scenario: Frame at playhead 0
- **WHEN** the user loads a video and the playhead is at 0 seconds
- **THEN** the preview area displays the first frame of the video

#### Scenario: User scrubs to mid-video
- **WHEN** the user moves the playhead to 30 seconds on a 60-second source
- **THEN** the preview area updates to display the frame nearest to 30 seconds

### Requirement: Render draggable 9:16 crop overlay

The system SHALL overlay a rectangular crop window on the preview area, with aspect ratio locked to 9:16 (portrait), that the user can move and resize.

The crop window MUST remain entirely within the bounds of the source frame at all times; if the user drags the window such that part of it would leave the source frame, the system MUST clamp the position so the entire window stays inside.

The crop window's coordinates MUST be stored in source pixel space, not in preview-window pixel space, so that the crop survives any change to the preview area's display size.

The crop window's aspect ratio MUST remain 9:16 at all times; resizing one edge MUST resize the other edge proportionally.

#### Scenario: Default crop is centered
- **WHEN** the user loads a 16:9 source video and the crop overlay first appears
- **THEN** the crop window is centered horizontally, fills the full source height, and has a width equal to height × 9/16

#### Scenario: User drags crop to the left
- **WHEN** the user clicks inside the crop window and drags it 100 pixels to the left
- **THEN** the crop window's stored source coordinates shift left by 100 pixels and the overlay re-renders at the new position

#### Scenario: User drags crop out of bounds
- **WHEN** the user drags the crop window such that its right edge would exit the source frame
- **THEN** the system clamps the position so the right edge stays at the source frame's right edge

#### Scenario: User resizes crop
- **WHEN** the user drags the bottom-right resize handle of the crop window downward
- **THEN** both the height and the width of the crop window grow so the aspect ratio stays 9:16

### Requirement: Play cut region in real time

The system SHALL provide a "Play" control that streams the decoded frames of the cut region (from IN to OUT) to the preview area in real time, advancing the playhead at the source's native frame rate.

While the cut region is playing, the system SHALL update the preview frame and the playhead position continuously; the system MUST stop playback automatically when the playhead reaches OUT.

The user MUST be able to stop playback at any time via a "Stop" control or by clicking the timeline.

#### Scenario: User plays the cut region
- **WHEN** the user clicks Play with IN at 30 seconds and OUT at 45 seconds on a 30 fps source
- **THEN** the preview shows the frame at 30 seconds, advances at 30 fps, and stops automatically when the playhead reaches 45 seconds

#### Scenario: User stops playback mid-region
- **WHEN** the user clicks Stop while the cut region is playing and the playhead is at 37 seconds
- **THEN** playback halts and the preview shows the frame nearest to 37 seconds
