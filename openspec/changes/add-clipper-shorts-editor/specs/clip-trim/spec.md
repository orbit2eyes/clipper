## ADDED Requirements

### Requirement: Display source duration and current playhead

The system SHALL display the source video's total duration and the current playhead time in a timeline area of the main window.

The playhead time MUST be formatted as `HH:MM:SS.mmm` (or `MM:SS.mmm` if duration is under one hour).

#### Scenario: User loads a 45-minute video
- **WHEN** the user loads a source with duration 45 minutes 12 seconds
- **THEN** the timeline area shows `45:12.000` as the total duration

### Requirement: Scrub the playhead by dragging

The system SHALL allow the user to drag the playhead along the timeline to seek to any position within the source's duration.

Seeking MUST update the preview area to show the frame at the new playhead position.

The playhead MUST NOT move outside the range `[0, duration]`.

#### Scenario: User drags playhead to 1:30
- **WHEN** the user clicks on the timeline at the position corresponding to 1 minute 30 seconds and drags
- **THEN** the playhead moves to 1:30, the preview area updates to that frame, and the displayed time reads `01:30.000`

#### Scenario: User drags past the end
- **WHEN** the user drags the playhead past the right edge of the timeline
- **THEN** the playhead clamps to the source's total duration

### Requirement: Set IN and OUT markers

The system SHALL provide controls to set the IN and OUT markers at the current playhead position.

The system SHALL display the current IN and OUT values as time codes.

IN MUST default to `0`. OUT MUST default to the source's total duration. The system MUST enforce `IN < OUT`; if the user attempts to set OUT to a value less than or equal to IN, the system MUST reject the operation and leave OUT unchanged.

IN and OUT MUST be quantized to the nearest frame boundary, using the source's frame rate as the unit. For a 30 fps source, a playhead at 1.0001 seconds rounds to 1.0333... seconds (frame 31, presented at 1s + 1/30s).

#### Scenario: User sets IN at current playhead
- **WHEN** the user moves the playhead to 30 seconds and clicks the "Set IN" control
- **THEN** the IN marker value updates to 30 seconds (or the nearest frame) and the displayed IN time reads accordingly

#### Scenario: User tries to set OUT before IN
- **WHEN** the user sets IN to 30 seconds, moves the playhead to 20 seconds, and clicks the "Set OUT" control
- **THEN** the system rejects the operation, OUT remains at its previous value, and the user sees a brief notice explaining the constraint

#### Scenario: IN is frame-aligned on a 30 fps source
- **WHEN** the user sets the playhead to 1.012 seconds on a 30 fps source and clicks "Set IN"
- **THEN** IN becomes `1.0333...` seconds (frame 31) so IN aligns to a frame boundary

### Requirement: Highlight cut region on the timeline

The system SHALL visually distinguish the region between IN and OUT on the timeline from the regions before IN and after OUT.

#### Scenario: IN at 30 s, OUT at 45 s on a 60 s source
- **WHEN** the user has set IN to 30 seconds and OUT to 45 seconds on a 60-second source
- **THEN** the timeline shows a highlighted band from 30 s to 45 s, with the rest of the timeline visually de-emphasized
