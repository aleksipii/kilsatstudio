# Native estimated mouth timing

The desktop Rhubarb panel runs only after **Estimate prepared line** is clicked.
First prepare the selected dialogue line with Kokoro or imported WAV audio. Supply
an explicit absolute path to an independently installed Rhubarb executable and
choose PocketSphinx (English) or Phonetic. The application does not install or
download the tool, recognizer, model or voices.

One persistent worker executes the existing bounded Rhubarb backend against the
prepared WAV and transcript. File reads, hashing and process execution stay off
the UI thread. The backend stages bounded inputs, fingerprints the executable
and exact decoded mono 24 kHz PCM, caps output, times out, and kills/reaps its
child on cancellation. The worker additionally checks the result against the
prepared audio buffer, rejecting a replaced WAV even when duration is unchanged.
Only one outstanding operation/result is accepted. Closing or cancelling cancels
its token; the UI waits for completion before allowing another submission.

Generation, line ID, transcript and WAV path identify the eligible preview.
Changing any of these discards stale completions and pending applications.
**Apply estimated timing** is a separate action: it imports the exact generated
JSON and reprepares the same audio through the normal alignment path. Output over
the editor's 100 KB input limit produces an actionable shorter-line error.

These are Rhubarb estimated mouth shapes, never forced-aligned phonemes. Stub
worker and headless UI tests establish bounded scheduling, cancellation and stale
state behavior. They do not establish actual recognizer execution, intelligibility,
perceptual lip-sync quality, macOS device playback or redistribution rights.
A real, licensed local installation and representative Mac audio/video comparisons
remain acceptance requirements.

Compare saved estimated cues with manual mouth-shape annotations using the
[offline quality evaluator](viseme-quality.md). This records agreement and
transition errors on the exact audio; it does not grant quality acceptance.
