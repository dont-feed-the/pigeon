# Independent English usability retake

Date: 2026-10-17. Used a fresh one-use browser login against the restarted isolated daemon, actual HTTP pages/forms, and the saved settings from the original session. No implementation source was read; no repository files were edited. This is HTTP/text testing, not visual, JavaScript interaction, hardware, or throughput testing. No CLI/config inspection was needed in this retake.

## Results

Saved-value confirmations: re-saved the upload cap, full-speed window, and battery threshold through Settings. The previous `setting-` prefix was absent in all three observed confirmations and page titles:

- “Maximum upload (auto or 2 MB/s): 2 MB/s.”
- “Full-speed windows (empty for none): 22:00-23:00.”
- “Low battery threshold (%): 35.”

Full-speed/cap interpretation: Settings now says, “Rates use bytes per second; full-speed windows use local time HH:MM-HH:MM, separated by commas. Full-speed windows turn off automatic slowing; your upload and download limits still apply.” I can now explain that the configured 22:00–23:00 window uses local time and the manual 2 MB/s upload cap still applies during it. This directly resolves the earlier cap-precedence ambiguity. “Local time” does not name a timezone or explicitly distinguish this computer's clock from the browser's clock, so a remote user could still ask which clock applies. I did not observe scheduled runtime behavior.

Automatic waits versus a person's Pause: Settings now says, “When set to Wait, file transfers wait below the battery threshold or in low-power mode, then resume automatically when conditions improve. Metered-network waits also end automatically. Share again resumes a pause you chose.” With threshold 35%, I can explain from the UI that Wait covers battery below 35% or low-power mode and ends automatically when conditions improve; a metered-network wait also ends automatically. I tested a chosen pause by selecting “until I resume” and submitting Pause. The response said “Sharing is paused until you resume it.” and “Sharing is paused. Changes are saved on this computer only, until you resume.” It offered a real Share again form. Submitting that form returned “Sharing again.” This makes the distinction clear. I did not simulate battery, low-power, or metered-network transitions and cannot claim those automatic transitions were observed. The UI also does not explicitly say whether a full-speed window overrides these waits; “automatic slowing” might leave that edge case open.

Invalid upload and recovery: submitted `2 bananas/s`. The response now says, ““2 bananas/s” is not valid for Maximum upload (auto or 2 MB/s). Nothing changed. Go back and use a listed choice or the format shown for that setting.” It uses the visible label, confirms preservation, and no longer directs me to the CLI or exposes `resources.upload`. Followed the Back link to Settings. The upload remained `2 MB/s`, window remained `22:00-23:00`, and battery threshold remained `35`. Recovery succeeded. The response is still a separate HTTP 200 page, with Back rather than an editable field. The rate format is available in the visible field label, so this did not block recovery.

## Outcome and limits

All requested retake checks were completed through the UI. The observed confirmation defect is fixed, cap precedence is clear, and automatic-wait versus chosen-pause wording is substantially clearer. Remaining minor ambiguity concerns which clock “local time” refers to and whether full-speed windows affect battery/metered waits. Final retained resource values are the same values observed after returning to Settings: Gentle, 2 MB/s upload, 22:00–23:00 full-speed window, 35% battery threshold, Wait for battery and metered network. The manual pause was resumed before finishing.
