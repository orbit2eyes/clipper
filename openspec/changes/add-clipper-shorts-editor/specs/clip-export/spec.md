## ADDED Requirements

### Requirement: Export a Shorts-ready MP4

The system SHALL export the cut region (from IN to OUT) of the loaded source as a 1080x1920 H.264 + AAC MP4 file, applying the user's crop window to convert the source aspect to 9:16 portrait.

The output container MUST be MP4. The video stream MUST be H.264 with yuv420p pixel format, 1080x1920 resolution, and constant frame rate equal to the source's frame rate. The audio stream MUST be AAC-LC; if the source has no audio, the output MUST have no audio stream.

The output file MUST be written with the `+faststart` flag set so the MP4 is streamable from the beginning.

The system MUST invoke ffmpeg as a child process for the encode; the system MUST NOT reimplement the encoder in Rust.

#### Scenario: Source is 16:9 with AAC audio
- **WHEN** the user exports with IN at 30 s, OUT at 45 s, and a 9:16 crop window centered on a 16:9 H.264+AAC source
- **THEN** the output is an MP4 with H.264 video at 1080x1920, AAC audio, duration 15 s, and `+faststart` flag set

#### Scenario: Source has no audio stream
- **WHEN** the user exports a cut region from a source that has no audio stream
- **THEN** the output MP4 has a video stream and no audio stream, and is still playable

### Requirement: Auto-number output filenames

The system SHALL write the exported file as `short_NNN.mp4` in the user-chosen output directory, where NNN is a zero-padded three-digit integer.

The system SHALL determine NNN by scanning the output directory for existing files matching the pattern `short_\d+\.mp4`, finding the highest existing NNN, and incrementing by one. If no matching files exist, NNN SHALL be `001`.

#### Scenario: Empty output directory
- **WHEN** the user chooses an empty output directory and exports
- **THEN** the output file is named `short_001.mp4`

#### Scenario: Directory contains short_001 and short_007
- **WHEN** the user chooses an output directory containing `short_001.mp4` and `short_007.mp4` and exports
- **THEN** the output file is named `short_008.mp4`

#### Scenario: User picks the same directory twice
- **WHEN** the user exports once, then exports again with the same output directory
- **THEN** the first export creates `short_001.mp4` and the second creates `short_002.mp4`

### Requirement: Report progress during export

The system SHALL display a progress indicator while ffmpeg is encoding, driven by ffmpeg's `-progress pipe:1` output.

The progress indicator MUST update at least once per second during the encode.

#### Scenario: Encoding a 60-second cut from a 30-minute source
- **WHEN** the user exports a 60-second cut region and the encode takes 30 seconds
- **THEN** the progress indicator updates from 0% to 100% over those 30 seconds, with at least one update per second

### Requirement: Allow export cancellation

The system SHALL allow the user to cancel an in-progress export. On cancel, the system MUST terminate the ffmpeg child process and MUST delete any partial output file the ffmpeg process created.

#### Scenario: User cancels mid-export
- **WHEN** the user clicks the cancel control while ffmpeg is encoding and has produced a partial `short_005.mp4`
- **THEN** the ffmpeg child process is terminated and `short_005.mp4` is removed from the output directory

### Requirement: Reject export with invalid state

The system MUST refuse to start an export if any of the following are true: no source is loaded, IN is greater than or equal to OUT, the crop window is empty, or the output directory is not writable.

The system SHALL display a message naming the missing or invalid item.

#### Scenario: User clicks Export without setting OUT
- **WHEN** the user clicks Export with IN at 30 seconds and OUT still at the default value of source duration, and source duration is 45 seconds, so IN < OUT
- **THEN** the export proceeds

#### Scenario: User clicks Export with IN equal to OUT
- **WHEN** the user clicks Export with IN at 30 seconds and OUT at 30 seconds
- **THEN** the system does not start ffmpeg and displays a message that OUT must be greater than IN
