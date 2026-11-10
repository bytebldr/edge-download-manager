# Code Signing Policy

> **Attribution Notice**:  
> **Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).**

---

## 1. Purpose & Scope

This Code Signing Policy governs the issuance, authorization, verification, and distribution of digitally signed software binaries for **Edge Download Manager (EDM)**.

Digital signatures provide cryptographic assurance to our users that:
1. **Authenticity**: The binaries were produced and published by the official Edge Download Manager project.
2. **Integrity**: The software has not been altered, corrupted, or tampered with by any intermediary or malicious third party since compilation.

---

## 2. Certificate Information

- **Signer / Publisher**: SignPath Foundation
- **Program Name**: Edge Download Manager (EDM)
- **Target Platform**: Windows 10 & Windows 11 (x64)
- **Artifacts Covered**:
  - `EDM_<version>_x64-setup.exe` (NSIS Windows Installer)
  - `EDM.exe` / `edm-desktop-app.exe` (Main Desktop Application Binary)
  - `edm-nmh.exe` (Native Messaging Host Browser Bridge)
  - `edm-cli.exe` (Command-Line Interface Downloader)

---

## 3. Team Roles and Release Authorization

To maintain the highest level of supply chain security, only authorized team members may initiate, review, and approve code signing requests.

| Role | Responsibilities | Assigned Members |
| :--- | :--- | :--- |
| **Project Lead & Owner** | Repository administration, signing request submission, release tagging, security oversight. | **Suvesh** ([@bytebldr](https://github.com/bytebldr)) |
| **Release Maintainers** | Code review, pre-flight invariant verification, changelog authoring, release publication. | **Edge Download Manager Core Team** |
| **Reviewers & Approvers** | Review of all code changes via pull request, verifying reproducible build integrity. | Core Maintainers |

### Security Invariants for Signing Roles
- **Multi-Factor Authentication (MFA)**: Mandatory hardware token or authenticator app 2FA across all GitHub and SignPath accounts for all team members with write or signing permissions.
- **Principle of Least Privilege**: Access to signing triggers and credentials is strictly limited to authorized maintainers.
- **Tamper-Evident History**: Direct pushing to release tags without passing automated CI checks is strictly prohibited.

---

## 4. Build Pipeline & Security Controls

All production binaries submitted for digital signing must strictly adhere to the following supply chain controls:

1. **GitHub-Hosted Automated Runners**:
   - Binaries are compiled exclusively on standard, ephemeral GitHub-hosted runners (`windows-latest`) triggered by authenticated Git release tags (`v*.*.*`).
   - Self-hosted or unverified local build machines are strictly barred from submitting production signing requests.

2. **Clean Room Verification**:
   - Every build executes `verify-invariants.ps1` and the complete Rust workspace test suite (`cargo test --workspace`) prior to packaging.
   - Zero compiler warnings, zero clippy violations, and zero invariant failures are tolerated on release builds.

3. **Cryptographic Build Provenance**:
   - Artifacts submitted to SignPath are cryptographically linked to the exact Git commit SHA and tag in the public repository [github.com/bytebldr/edge-download-manager](https://github.com/bytebldr/edge-download-manager).
   - Artifact checksums (SHA-256) are calculated and published alongside every release on GitHub Releases.

4. **Zero-Malware & Zero-PUP Invariant**:
   - Edge Download Manager is strictly free of malware, spyware, adware, telemetry trackers, and Potentially Unwanted Programs (PUPs).
   - The application does not bundle third-party offers or unsolicited auxiliary software.

---

## 5. Revocation & Incident Response

In the improbable event of a compromised signing credential, unauthorized release artifact, or discovered security defect:

1. The project maintainers will immediately notify the **SignPath Foundation** to revoke or suspend the affected certificate or signing requests.
2. A public security advisory will be published in the [GitHub Security Advisories](https://github.com/bytebldr/edge-download-manager/security/advisories) registry.
3. Users and package managers will be advised immediately through repository releases and release notes.

To report security concerns regarding signed binaries, refer to our [Security Policy](SECURITY.md).
