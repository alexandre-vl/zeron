# Self-hosted servers: screens

Images for the pull request "Self-hosted servers: chosen on desktop, iPhone
and Android, shared by an invite link". They're kept on this branch, not in the
pull request's diff.

"Acme" is a stand-in server: a static `/.well-known/zeron`
(`{"zeron":1,"name":"Acme","auth":{"mode":"workos","workosClientId":"client_01ACMEFAKE"}}`)
served by `python3 -m http.server 8899`. Desktop shots use a fixture's
stand-in, so no network is involved.

## Desktop (zgx)

`desktop-*.png`: the sign-in window from `apps/zgx/ui/signin_server_fixture.luau`,
680x500 at 2x, dark. They're rendered offscreen by `apps/zgx/src/audit_shots.rs`.
The fixture's commands set each state: `signin_server.ask` opens the dialog,
`signin_server.invite` follows an invite link signed out, and
`signin_server.signed_in` follows one signed in.

```bash
cd apps/zgx
AUDIT_OUT=/tmp/shots AUDIT_SHOTS='signin_server@680x500|!signin_server.invite|+300|~3-invite' \
  cargo test --bin zeron-zgx audit_shots -- --ignored --nocapture
```

## iPhone

`ios-*.png`: attachments from `apps/ios/ZeronUITests/ServerFlowTests.swift`,
run on an iOS simulator (iPhone 18 Pro) and exported from the `.xcresult`.
The tests open the welcome's ⋯ menu, the server sheet, and an invite link
(`zeron://server?address=…`), then Settings.

## Android

`android-*.png`: the debug app on an Android 16 emulator (Pixel 9 Pro).
`10.0.2.2:8899` is the host machine as the emulator sees it. The invite link
was opened with:

```bash
adb shell am start -a android.intent.action.VIEW \
  -d 'zeron://server?address=http%3A%2F%2F10.0.2.2%3A8899' sh.zeron.android
```

## Invite page

`page-*.png`: what `edge/src/join.ts` serves at `/`, rendered with headless
Chrome at 720 px wide and 2x. The light and dark versions force
`prefers-color-scheme`. The phone version is the page inside a 390 px frame.
`page-not-ready.png` is an edge whose sign-in isn't configured (503).
