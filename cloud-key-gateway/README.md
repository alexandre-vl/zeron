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

From the earlier pull request (#1086); the iOS code they show
(`apps/ios/Zeron/Cloud/`) is unchanged since. A local simulator harness
(iOS 27, iPhone 18 Pro) compiled the app's own Swift against stand-ins for
the three FFI calls.
