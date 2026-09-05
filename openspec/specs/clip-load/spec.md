## Purpose

TBD — see archived change for original Why/Capabilities context.

## Requirements

### Requirement: Open a video file

The system SHALL allow the user to select a video file from the local filesystem and load it into the editing workspace.

The system SHALL accept file paths provided through any of the following: a file picker dialog, drag-and-drop onto the main window, or a path passed on the command line.

The system SHALL read the file by streaming it through ffmpeg's demuxer; the file MUST NOT be fully loaded into memory.

#### Scenario: User picks a valid MP4
- **WHEN** the user selects a `.mp4` file with H.264 video and AAC audio via the file picker
- **THEN** the system loads the file and the workspace displays its duration, resolution, frame rate, codec, and a preview of the first frame

#### Scenario: User drags a file onto the window
- **WHEN** the user drags a supported video file from their file manager and drops it onto the main window
- **THEN** the system loads the file as if the user had picked it through the file picker

#### Scenario: Unsupported file extension
- **WHEN** the user selects a file the demuxer cannot decode (e.g., a `.txt` file or a corrupt `.mp4`)
- **THEN** the system displays an error message naming the file and the reason it could not be loaded, and the workspace remains in its previous state

### Requirement: Parse source metadata

The system SHALL probe the loaded video file with ffmpeg and surface the following metadata to the rest of the app: container format, video codec name and profile, video resolution (width and height in pixels), frame rate as a rational number, total duration in seconds, audio codec name and channel count if an audio stream is present.

The system SHALL treat metadata as immutable for the lifetime of the loaded source; if the user loads a new file, the metadata is replaced.

#### Scenario: Source has video and audio
- **WHEN** the user loads a file containing one H.264 video stream and one AAC stereo audio stream
- **THEN** the system exposes the video resolution, frame rate, duration, video codec, audio codec, and audio channel count to the rest of the app

#### Scenario: Source has video only, no audio
- **WHEN** the user loads a file containing a video stream but no audio stream
- **THEN** the system exposes the video metadata and the absence of audio to the rest of the app

### Requirement: Reject files ffmpeg cannot read

The system SHALL refuse to load any file that ffmpeg's demuxer cannot read and SHALL surface the ffmpeg error message to the user.

#### Scenario: Corrupt container
- **WHEN** the user picks a file whose container header is valid but whose media data is truncated or corrupt
- **THEN** the system displays an error that includes ffmpeg's diagnostic text and does not enter the editing workspace
