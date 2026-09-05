## MODIFIED Requirements

### Requirement: Display source duration and current playhead

The system SHALL display the source video's total duration and the current playhead time in a timeline area of the main window.

The playhead time MUST be formatted as `HH:MM:SS.mmm` (or `MM:SS.mmm` if duration is under one hour).

The system SHALL also render a horizontal timeline strip below the preview area showing the source duration as the strip's full width, with the playhead as a vertical line at the corresponding horizontal position, with IN and OUT markers as triangular handles at the top edge of the strip, and with the region between IN and OUT visually highlighted.

#### Scenario: User loads a 45-minute video
- **WHEN** the user loads a source with duration 45 minutes 12 seconds
- **THEN** the timeline area shows `45:12.000` as the total duration

#### Scenario: Timeline strip renders cut region and markers
- **WHEN** the user has set IN to 30 seconds and OUT to 45 seconds on a 60-second source
- **THEN** the timeline strip shows a highlighted band from the 30-second position to the 45-second position, a yellow IN triangle at the 30-second position, a yellow OUT triangle at the 45-second position, and a white playhead line at the current playhead's horizontal position

### Requirement: Scrub the playhead by dragging

The system SHALL allow the user to drag the playhead along the timeline to seek to any position within the source's duration.

The system SHALL also allow the user to click on the timeline strip's background to seek the playhead to the corresponding time, and to drag on the strip's background for continuous seek.

Seeking MUST update the preview area to show the frame at the new playhead position.

When the source's frame rate is known and positive, every seek derived from the timeline strip MUST be quantized to the nearest frame boundary using the source's frame rate as the unit.

The system MUST stop any in-progress playback when the user seeks via the timeline strip.

The playhead MUST NOT move outside the range `[0, duration]`.

#### Scenario: User drags playhead to 1:30
- **WHEN** the user clicks on the timeline at the position corresponding to 1 minute 30 seconds and drags
- **THEN** the playhead moves to 1:30, the preview area updates to that frame, and the displayed time reads `01:30.000`

#### Scenario: User drags past the end
- **WHEN** the user drags the playhead past the right edge of the timeline
- **THEN** the playhead clamps to the source's total duration

#### Scenario: User clicks on the timeline strip to seek
- **WHEN** the user clicks on the timeline strip background at the horizontal position corresponding to 2 minutes 15 seconds
- **THEN** the playhead moves to 2:15 (or the nearest frame on a 30 fps source), the preview area updates to that frame, and any in-progress playback stops

#### Scenario: Timeline strip click is frame-aligned on a 29.97 fps source
- **WHEN** the user clicks the timeline strip at the horizontal position corresponding to 1.012 seconds on a 29.97 fps source
- **THEN** the playhead snaps to the nearest frame boundary (approximately 1.0003 or 1.0337 seconds) so the seek aligns to a frame

### Requirement: Set IN and OUT markers

The system SHALL provide controls to set the IN and OUT markers at the current playhead position.

The system SHALL also allow the user to drag the IN triangle on the timeline strip to set the IN marker, and to drag the OUT triangle to set the OUT marker.

The system SHALL display the current IN and OUT values as time codes.

IN MUST default to `0`. OUT MUST default to the source's total duration. The system MUST enforce `IN < OUT`; if the user attempts to set OUT to a value less than or equal to IN, the system MUST reject the operation and leave OUT unchanged.

IN and OUT MUST be quantized to the nearest frame boundary, using the source's frame rate as the unit. For a 30 fps source, a playhead at 1.0001 seconds rounds to 1.0333... seconds (frame 31, presented at 1s + 1/30s).

When dragging the IN triangle, the resulting IN value MUST be clamped to the range `[0, OUT]`. When dragging the OUT triangle, the resulting OUT value MUST be clamped to the range `[IN, duration]`.

The system MUST stop any in-progress playback when the user updates IN or OUT via triangle drag.

#### Scenario: User sets IN at current playhead
- **WHEN** the user moves the playhead to 30 seconds and clicks the "Set IN" control
- **THEN** the IN marker value updates to 30 seconds (or the nearest frame) and the displayed IN time reads accordingly

#### Scenario: User tries to set OUT before IN
- **WHEN** the user sets IN to 30 seconds, moves the playhead to 20 seconds, and clicks the "Set OUT" control
- **THEN** the system rejects the operation, OUT remains at its previous value, and the user sees a brief notice explaining the constraint

#### Scenario: IN is frame-aligned on a 30 fps source
- **WHEN** the user sets the playhead to 1.012 seconds on a 30 fps source and clicks "Set IN"
- **THEN** IN becomes `1.0333...` seconds (frame 31) so IN aligns to a frame boundary

#### Scenario: User drags IN triangle to 12 seconds
- **WHEN** the user drags the IN triangle on the timeline strip to the horizontal position corresponding to 12 seconds
- **THEN** the IN marker updates to 12 seconds (or the nearest frame), the displayed IN time reads accordingly, and any in-progress playback stops

#### Scenario: User drags IN triangle past OUT
- **WHEN** the user drags the IN triangle to the right past the OUT marker
- **THEN** IN clamps to OUT's current value (no swap); OUT remains unchanged

#### Scenario: User drags OUT triangle to 48 seconds
- **WHEN** the user drags the OUT triangle on the timeline strip to the horizontal position corresponding to 48 seconds
- **THEN** the OUT marker updates to 48 seconds (or the nearest frame), the displayed OUT time reads accordingly, and any in-progress playback stops
