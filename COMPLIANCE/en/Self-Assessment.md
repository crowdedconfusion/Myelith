# Self-Assessment under the AI Act

**Last reviewed:** 2026-09-25 · **Deutsch:** [Selbsteinschaetzung.md](../de/Selbsteinschaetzung.md)

> ⚠️ **Self-assessment, not legal advice.** The provider prepared this
> overview based on their own best understanding of Regulation (EU)
> 2024/1689 and the Commission's guidelines. It does not replace review by
> a lawyer or a decision by an authority. The German version is
> authoritative.

## Classification in brief

| Question | Answer |
|---|---|
| Who is the provider? | Joschka Benjamin Hänsler, private individual, Germany, non-commercial |
| What is provided? | An **AI system** (the local assistant Myelith: window, console, command line) and **modifications of general-purpose AI models** (the Myelith artefacts) |
| High-risk? | **No.** The intended use policy excludes every area in Annex III |
| GPAI with systemic risk? | **No.** Far below the threshold of 10²⁵ floating-point operations |
| Exemption for free and open-source licences? | **Not claimed.** The PolyForm Shield License 1.0.0 restricts use for competing products and is therefore not a free and open-source licence within the meaning of Art. 2(12), Art. 53(2) and Recital 102 |
| Role for GPAI obligations | Provider with all obligations under Art. 53 for its own modifications, because Myelith fine-tunes them itself (provider's decision) |

## Overview by article

Status: ✅ implemented · 🟡 partially implemented · ➖ not applicable

| Article | Subject | Status | Implementation or reason |
|---|---|---|---|
| Art. 2 | Scope | ✅ | Myelith is treated as made available in the Union as a precaution; the exemption for free and open-source licences does not apply (see above) |
| Art. 4 | AI literacy | 🟡 | The duty applies to staff of providers and deployers; the provider is an individual. Users learn capabilities and limits from the start-up notice and the [intended use policy](Intended-Use-Policy.md) |
| Art. 5 | Prohibited practices | ✅ | [Intended use policy](Intended-Use-Policy.md) section 2.1; request filter at every entry point (`CLIENT/myl-client/src/schutzfilter.rs`), refusals in the action log without plain text; rule for the vision model (`CLIENT/myl-senses/src/sehen.rs`, `SEHREGEL`). Limit: a filter can be circumvented by rephrasing |
| Art. 6, Annex III | Classification as high-risk | ➖ | Not intended for Annex III areas ([intended use policy](Intended-Use-Policy.md) section 2.2). Anyone using Myelith there changes the intended purpose (Art. 25) |
| Art. 8 to 15 | Requirements for high-risk systems | ➖ | Not applicable. **Voluntarily** modelled on them: action log (Art. 12), confirmation before actions and emergency stop (Art. 14) |
| Art. 12 (voluntary) | Record-keeping | ✅ | `CLIENT/myl-client/src/protokoll.rs`: every agent action as a JSON line with time, kind, tool, fingerprint of input and output (keyed SHA-256, no plain text), decision and result; 30 days; viewer in the window (settings, action log) and via `myl protokoll` |
| Art. 14 (voluntary) | Human oversight | ✅ | Default `agent.modus = manual`: writes, commands and web requests are presented before they run. Emergency stop in the window (button, ⌘. or Ctrl+.) and in the console (Ctrl-C or Esc during a run); it halts generation and every further tool, and the conversation is kept |
| Art. 16 to 22 | Obligations for high-risk systems, authorised representatives | ➖ | Not applicable. Art. 22 concerns providers established in third countries; the provider is in Germany |
| Art. 25 | Responsibilities along the value chain | ✅ | [Intended use policy](Intended-Use-Policy.md) section 2.2: whoever changes the intended purpose takes on provider obligations |
| Art. 50(1) | Disclosure of AI interaction | ✅ | Notice at **every** start, to be actively acknowledged (window and console), a line on standard error for `myl`; permanent AI label in the window header and the console footer; every answer in the window carries "AI-generated" (`CLIENT/myl-client/src/kennzeichnung.rs`) |
| Art. 50(2) | Marking of synthetic content | 🟡 | **Speech ✅**: every generated audio file carries metadata (XMP with the IPTC source type `trainedAlgorithmicMedia`, RIFF INFO) and a watermark in the signal before it is played or stored; anything that cannot be marked is not played (`CLIENT/myl-senses/src/kennzeichnung.rs`); verifiable with `myl kennzeichen <file>`. **Text 🟡**: marked in the interface, not yet machine-readable in files the agent writes. **Image, video ➖**: not generated. **C2PA signature**: not yet implemented |
| Art. 50(3) | Emotion recognition, biometric categorisation | ➖ | Not offered and excluded (Art. 5) |
| Art. 50(4) | Deep fakes | ✅ | Duty of deployers; supported by marking every generated voice and by consent when a voice sample is uploaded (checked in the window and in the command) |
| Art. 51, 52, 55 | GPAI with systemic risk | ➖ | Below the threshold |
| Art. 53(1)(a), (b) | Technical documentation | ✅ | [GPAI documentation](GPAI-Documentation.md) (Annexes XI and XII, Code of Practice form) |
| Art. 53(1)(c) | Copyright policy | ✅ | [Copyright policy](Copyright-Policy.md) |
| Art. 53(1)(d) | Summary of training content | ✅ | [Training data](Training-Data.md) (AI Office template) |
| Art. 54 | Authorised representatives for GPAI | ➖ | Provider established in the Union |

## Other areas of law touched here

| Area | Status |
|---|---|
| Data protection (GDPR) | Myelith processes everything on the user's computer. A voice sample is personal data; it is stored only with confirmed consent and never leaves the computer. The action log contains no plain text |
| Licences of third-party components | [NOTICES](../NOTICES.md), generated and checked in CI. Open: the wordmark typeface (licence unclear) |

## Open items

1. **C2PA manifest** for generated audio files (requires a signing certificate).
2. **Machine-readable marking of texts** the agent writes to files.
3. **Licence of the wordmark typeface**: clarify, or remove the embedded subset.
4. **Review by a lawyer** of this assessment.

## Review

This self-assessment is reviewed whenever a change touches one of the
items above, and at least every six months.
