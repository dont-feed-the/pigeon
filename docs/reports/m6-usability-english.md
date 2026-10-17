# Independent English usability session

Date: 2026-10-17. Method: live HTTP requests, HTML text/form parsing, a cookie jar, and normal Settings form submissions against the real isolated daemon. I approached the tasks as an English-speaking user who did not build pigeon. I did not read implementation source or modify repository files. This was HTTP/text usability testing, not visual testing, JavaScript interaction testing, throughput measurement, or laptop hardware validation.

The one-use login worked and landed on Home. Home said “This computer is not a member of the group.” with “Code: P-NOT-IN-GROUP”. The navigation included Settings, which remained usable without joining a group.

## Task observations before CLI/config verification

| Task | Actions and observed outcome | User explanation / remaining uncertainty |
| --- | --- | --- |
| Keep sharing discreetly on a laptop | Found “Use of this computer” in Settings and “Gentle keeps sharing in the background.” Gentle was initially selected; saved it and received “Preset: Gentle.” | The background-sharing wording made Gentle an easy choice. I did not use the separate Pause control because the task was to keep sharing. No actual sharing or laptop impact was tested. |
| Cap uploads at 2 MB/s | Entered `2 MB/s` in “Maximum upload (auto or 2 MB/s)” and saved. Returning to Settings showed `2 MB/s`. | The example and bytes-per-second explanation were enough to enter the intended value. Success text was visibly defective: “Maximum upload (auto or 2 MB/s): setting-2 MB/s.” |
| Switch presets and explain manual cap | Changed Gentle to Fast, then returned to Settings. Fast was selected while upload still read `2 MB/s`; changed back to Gentle afterward. | “Details you set override the preset” and the retained field made me expect the manual cap to survive preset changes. Fast also changed the displayed battery behavior from Wait/20% to Continue/0% and metered behavior from Wait to Continue. The form does not explain those preset effects before choosing Fast. |
| Full speed 22:00–23:00; does cap apply? | Entered `22:00-23:00` in “Full-speed windows (empty for none)” and saved. Returning to Settings showed this window and the upload cap together. | I could configure the window but could not confidently explain its precedence from the page. “Full-speed” suggests the cap may be removed then, while “Details you set override the preset” may suggest my explicit cap remains. The page supplies time syntax, not whether the window overrides manual caps, battery waits, or metered waits, nor which timezone applies. No claim of actual window-time behavior is made. |
| Invalid rate and recovery | Submitted `2 bananas/s`. The response said ““2 bananas/s” is not a value “resources.upload” takes.” followed by “See the values with: pigeon settings” and “Code: P-SETTING-VALUE”. Followed the visible Back destination `/settings`. Old upload `2 MB/s`, Gentle, and `22:00-23:00` remained. | Old settings were preserved and the Back path recovered successfully. The error shows an internal key and tells a web user to use the CLI. The response does not retain an editable form or present accepted rate syntax beside the error. It was HTTP 200 despite the validation failure. |
| Change battery threshold; explain wait/resume | Changed “Low battery threshold (%)” from 20 to 35. It saved. “When low on battery” remained Wait after returning to Gentle. | I would expect sharing to wait when the laptop is low at the chosen threshold and resume once that condition clears. The page does not say whether 35 itself triggers waiting, whether plugging in resumes immediately, whether recovery above the threshold resumes automatically, or how this interacts with a full-speed window. Those are expectations, not verified runtime behavior. |

## Exact visible defects and confusing text

1. Saved text values leak the prefix `setting-` into the page title and main confirmation. Observed: “Maximum upload (auto or 2 MB/s): setting-2 MB/s.”, “Full-speed windows (empty for none): setting-22:00-23:00.”, and “Low battery threshold (%): setting-35.” Preset confirmations were normal (“Preset: Fast.” / “Preset: Gentle.”).
2. Full-speed meaning and manual-cap precedence cannot be established from the Settings explanation. The observed explanation is exactly: “Gentle keeps sharing in the background. Details you set override the preset. Rates use bytes per second; full-speed windows use HH:MM-HH:MM, separated by commas.”
3. The invalid-rate recovery path works, but the message uses `resources.upload` instead of the visible field label and directs the user to `pigeon settings`. It supplies no direct accepted-value explanation beyond the existing label available after Back.
4. Battery “Wait” is short and understandable as a temporary condition, but resume conditions and threshold boundary are not explained.
5. Each submission in this HTTP session opened a separate confirmation/error page with a Back link. This was functional, but meant returning to Settings to see surrounding values. I did not test whether JavaScript presents a different inline experience.

## Verification only after completing UI tasks

Ran the supplied CLI against the same socket with `--json settings`. It confirmed:

- `resources.preset`: `gentle`
- `resources.upload`: `2 MB/s`
- `resources.download`: `auto`
- `resources.on_battery`: `pause`
- `resources.battery_percent`: `35`
- `resources.on_metered`: `pause`
- `resources.full_speed`: `22:00-23:00`
- `resources.scrub_window`: `01:00-06:00`

Read only the isolated daemon's `config/config.toml`, which contained `[resources]` with `preset = "gentle"`, `upload = "2 MB/s"`, `full_speed = ["22:00-23:00"]`, and `battery_percent = 35`. This confirms persistence of the manual cap, window, and changed threshold. It does not prove scheduling or runtime enforcement.

A final `--json status` returned `{"type":"Problem","code":"not-in-group","message":{"id":"problem-not-in-group"}}` with exit status 1. Therefore no active group transfer, effective throughput, or battery wait/resume was observed. No source changes, fixes, pushes, merges, or third-party messages were performed.
