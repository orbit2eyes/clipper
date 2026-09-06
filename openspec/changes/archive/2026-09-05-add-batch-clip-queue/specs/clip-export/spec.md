## ADDED Requirements

### Requirement: Manage a clip queue

The system SHALL maintain an ordered queue of clip definitions, where each clip definition is a pair `(IN, OUT)` of timestamps in seconds referencing positions on the loaded source.

The system SHALL provide an "Add at IN/OUT" control. The control computes a window size `W = out_marker - in_marker` (the difference between the current IN and OUT markers), then appends a new clip definition `(playhead, playhead + W)` to the queue, clamping the OUT time to the source's duration if needed. If `W` is less than or equal to zero at the moment the user invokes the control, the system MUST NOT add the clip and SHALL display a brief notice asking the user to set IN and OUT to define a clip length.

The system SHALL render the queue in a panel as a list of rows. Each row MUST display the clip's one-based index, the IN time, and the OUT time. Each row MUST provide a `×` control that removes that clip from the queue.

The system MUST persist the queue in memory only; restarting the application MUST discard the queue.

#### Scenario: User adds three clips at the same window size from different playheads
- **WHEN** the user sets IN to 0 s and OUT to 30 s, then scrubs to 30 s and clicks "Add at IN/OUT", then scrubs to 134 s and clicks "Add at IN/OUT", then scrubs to 902 s and clicks "Add at IN/OUT"
- **THEN** the queue contains three entries: `(30, 60)`, `(134, 164)`, `(902, 932)`, each with the same 30-second window size

#### Scenario: User attempts to add when window size is zero
- **WHEN** the user has not changed IN and OUT from their defaults (IN equals OUT) and clicks "Add at IN/OUT"
- **THEN** the queue is unchanged and the user sees a brief notice asking them to set IN and OUT to define a clip length

#### Scenario: User attempts to add when playhead is near source end
- **WHEN** the user has set IN and OUT to define a 30-second window, and the playhead is at 420 s of a 434.73 s source, and clicks "Add at IN/OUT"
- **THEN** the queue gains a clip with IN at 420 s and OUT at 434.73 s (clamped to source duration), and the user sees a brief notice that the clip was clamped

#### Scenario: User removes a clip via the row's `×` control
- **WHEN** the queue contains three entries and the user clicks `×` on the middle row
- **THEN** the queue contains two entries (the first and third); the second entry is gone

### Requirement: Batch export the clip queue

The system SHALL provide an "Export all" control that runs the export pipeline sequentially for every clip in the queue, producing one `short_NNN.mp4` file per clip in the user-chosen output directory.

While the batch is running, the "Add at IN/OUT" and "Export all" controls MUST be disabled.

Each clip in the batch SHALL be exported using the crop window and source that are current at the moment the clip's ffmpeg child process is spawned. The output file names SHALL be auto-incremented across the batch using the existing `next_short_name` rule, so the first clip of the batch is the next available `short_NNN.mp4`, the second is `short_(N+1).mp4`, and so on.

The system MUST use the existing ffmpeg child-process pipeline for each clip; the system MUST NOT introduce additional encoders or new export paths.

The batch SHALL terminate successfully when all clips have been exported, or be cancelled by the user before completion. On successful completion, the system SHALL display a "Saved N clips" message.

#### Scenario: User exports a queue of three clips
- **WHEN** the queue contains three clips with valid IN/OUT pairs and the user clicks "Export all"
- **THEN** the system runs the export pipeline three times in sequence, producing `short_NNN.mp4`, `short_(N+1).mp4`, and `short_(N+2).mp4` in the output directory, where N is the next available index based on existing files

#### Scenario: User clicks Export all with an empty queue
- **WHEN** the user clicks "Export all" while the queue is empty
- **THEN** the system does nothing (the control is disabled)

#### Scenario: Progress shows current clip index during a batch
- **WHEN** a batch of five clips is running and the second clip is being encoded
- **THEN** the progress display shows "Exporting clip 2 / 5" alongside the per-clip percentage

#### Scenario: Add and Export all are disabled while a batch runs
- **WHEN** the user has clicked "Export all" and the batch is in progress
- **THEN** the "Add at IN/OUT" control and the "Export all" control are both disabled

#### Scenario: Batch completes successfully
- **WHEN** the last clip of a batch finishes encoding
- **THEN** the queue is cleared from in-flight state (the user's persisted queue remains), the progress display clears, and the system displays "Saved N clips" where N is the number of clips in the completed batch

### Requirement: Cancel a running batch

The system SHALL allow the user to cancel a batch that is in progress. On cancel, the system MUST terminate the currently-running ffmpeg child process, MUST delete any partial output file that process created, and MUST drop all remaining queue entries from the in-flight batch.

The user's persisted queue (the list of `(IN, OUT)` pairs accumulated via "Add at IN/OUT") MUST remain intact across a cancel so the user can re-invoke "Export all" to retry.

#### Scenario: User cancels a batch mid-run
- **WHEN** the user clicks cancel while clip 2 of 5 is encoding
- **THEN** the ffmpeg child process for clip 2 is terminated, the partial output file is removed, and clips 3, 4, and 5 are dropped from the in-flight batch; the persisted queue still contains all five clip definitions
