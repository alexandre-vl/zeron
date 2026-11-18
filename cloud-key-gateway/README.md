# Cloud key gateway: screens

Images for the pull request "Cloud: an Anthropic API key can name its LLM gateway".
They live on this branch, not in the pull request's diff.

## Desktop (zgx)

The gallery's Providers example (`apps/zgx/ui/demos/example_providers.luau`):
the production `ds/provider_card.luau` dialog over the Settings window, at the
size Settings opens, on the example's synthetic accounts (their emails are
blurred by the gallery).

```bash
cd apps/zgx
PATH=/opt/homebrew/opt/zig@0.16/bin:$PATH cargo run -- gallery --inspect
bash drive.sh dark    # then click "Light" in the gallery and: bash drive.sh light
```

`drive.sh` (on this branch) drives the gallery through `zgx inspect`: Cloud,
the API keys, the Anthropic key, an `http://` base URL, the same with
`https://`, a trailing `/v1`, then `/anthropic/`, and Save. Each step is a
snapshot of the window, cropped to the Settings window (zoom 90 %, 1640x1170 px).
The GIFs are those frames at 1100 px wide, each held as long as the script
says; the MP4 is the dark run at full size.

## iPhone

`iphone-*.png`: after the pull request's third commit, the sheet in the
desktop's words and order. A local simulator harness (iOS 27, iPhone 18 Pro)
compiles the app's own `APIKeySheetViewController.swift` and `AccountCell`
against stand-ins for the three FFI calls, then opens the sheet in each state.

`ios-*.png` are the earlier pull request's (#1086), before that commit.

## Android

`android-*.png`, for the stacked Android pull request: the debug app in demo
mode (`--ez demo true --ez cloud true`) on an Android 16 emulator (Pixel 9
Pro), driven with `adb shell input`. The demo core judges a base URL as the
real one does; it doesn't store keys, so no saved key shows in its list.
Resized to 720 px wide.
