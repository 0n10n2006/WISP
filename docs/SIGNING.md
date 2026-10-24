# Code signing and antivirus false positives

Windows Defender and SmartScreen judge a new app mostly on two things: whether
the files are signed by a known publisher, and whether the app does anything
that looks like malware. Wisp 0.2.9 works on both.

## What changed in the app

- **No keyboard hook.** Hold-to-talk used a global low-level keyboard hook
  (`SetWindowsHookEx(WH_KEYBOARD_LL)`), which is the classic keylogger
  signature that heuristics flag. It now reads key presses through Raw Input
  (the API games use) on a hidden window. Raw Input can't block or change
  keys, and Wisp still only looks at modifier keys.
- **Full file details.** `wisp.exe` carries a product name, company,
  description, version and copyright (Properties → Details), so it no longer
  looks like an anonymous binary.
- **Plain, per-user installers.** The NSIS setup installs for the current user
  only (no admin prompt) and isn't packed or obfuscated.

## Free signing with SignPath Foundation (one-time setup)

SignPath Foundation signs open-source projects for free with its own
certificate. CI is already wired for it. The signing steps stay skipped
until the secrets below exist, so builds keep working in the meantime.

1. Apply at <https://signpath.org/apply> with the public repository URL. The
   project needs an OSI licence (MIT, see `LICENSE`) and must build in GitHub
   Actions, which it does.
2. Once you're approved, SignPath creates an organisation and project. Connect
   the project to this GitHub repository as a _Trusted Build System_ (GitHub.com).
3. Create an **artifact configuration** that signs the files inside the
   uploaded zip. For example:

   ```xml
   <artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
     <zip-file>
       <pe-file path="*-setup.exe">
         <authenticode-sign />
       </pe-file>
       <msi-file path="*.msi">
         <directory path="*">
           <pe-file path="wisp.exe"><authenticode-sign /></pe-file>
         </directory>
         <authenticode-sign />
       </msi-file>
     </zip-file>
   </artifact-configuration>
   ```

4. In GitHub → Settings → Secrets and variables → Actions add:
   - Secret `SIGNPATH_API_TOKEN`: an API token of a CI user with submitter
     rights.
   - Variable `SIGNPATH_ORGANIZATION_ID`.
   - Variable `SIGNPATH_PROJECT_SLUG`.
   - Variable `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`.
   - Variable `SIGNPATH_POLICY_SLUG` (optional). CI defaults to
     `test-signing`; releases default to `release-signing`.
5. Push. The Windows job uploads `wisp-windows-installer-signed`, and tagged
   releases ship the signed installers. The release notes say whether a build
   is signed.

For more supply-chain hardening, pin `signpath/github-action-submit-signing-request`
to a commit SHA in both workflows, as the other actions are.

SmartScreen reputation builds up per certificate as people download the signed
installers, so the "Windows protected your PC" prompt fades out over the first
releases.

## Reporting a false positive to Microsoft

If Defender flags a build:

1. Go to <https://www.microsoft.com/wdsi/filesubmission>, choose **Software
   developer** and **Incorrectly detected as malware/malicious**.
2. Upload the installer (or `wisp.exe`) and name the detection shown in
   Windows Security → Protection history.
3. Microsoft usually clears it within a few days. Updated definitions then
   stop flagging that file.
