# Intended Use Policy and Excluded Uses

**As of:** 2026-09-25 · **Deutsch:** [Zweckbestimmung.md](../de/Zweckbestimmung.md)

This policy defines what Myelith was built for and what it must not be
used for. It forms part of the terms of use within the meaning of
Regulation (EU) 2024/1689 (AI Act) and applies to every use of the
software, the models and the release bundles. The German version is
authoritative.

## 1. What Myelith is built for

Myelith is a **general-purpose AI assistant that runs locally**, with a
language model at its core. Intended uses:

- **Conversation and information**: answering questions, explaining,
  summarising and translating texts, developing ideas.
- **Working on your own files**: reading, changing and creating texts
  and source code in a folder the user explicitly releases.
- **Web research**, when the user turns it on.
- **Senses**: turning speech into text (hearing), describing images
  (seeing), reading answers aloud (speaking), optionally in a voice the
  user provides as a recording.
- **Taking part in the Myelith network**: performing and checking
  computational work as described in the white paper.

Myelith is meant for **private individuals and developers** who want to
run an assistant on their own computer.

## 2. Excluded uses

### 2.1 Prohibited practices under Article 5 AI Act

Myelith must **never** be used for:

1. **Subliminal, manipulative or deceptive influence** that materially
   distorts people's behaviour and causes harm (Art. 5(1)(a)).
2. **Exploiting vulnerabilities** due to age, disability or social or
   economic situation (Art. 5(1)(b)).
3. **Social scoring**: evaluating or classifying people by their social
   behaviour or personal characteristics with detrimental effect
   (Art. 5(1)(c)).
4. **Predicting criminal offences** based solely on profiling or
   personality traits (Art. 5(1)(d)).
5. **Untargeted scraping of facial images** from the internet or CCTV
   footage to build facial recognition databases (Art. 5(1)(e)).
6. **Emotion recognition in the workplace and in education**
   (Art. 5(1)(f)).
7. **Biometric categorisation** by race, political opinions, trade union
   membership, religious or philosophical beliefs, sex life or sexual
   orientation (Art. 5(1)(g)).
8. **Real-time remote biometric identification** in publicly accessible
   spaces (Art. 5(1)(h)).

### 2.2 High-risk areas under Annex III AI Act

Myelith is **not** intended for, and must not be used in, the following
areas. Anyone who nevertheless uses it there changes its intended purpose
and thereby takes on the obligations of a provider of a high-risk AI
system (Art. 25 AI Act).

1. **Biometrics**: remote identification, biometric categorisation,
   emotion recognition.
2. **Critical infrastructure**: safety components in the management and
   operation of digital infrastructure, road traffic, and the supply of
   water, gas, heating and electricity.
3. **Education and vocational training**: access and admission,
   evaluating learning outcomes, assessing the level of education,
   monitoring during tests.
4. **Employment and workers management**: selecting candidates, decisions
   on hiring, promotion and termination, allocating tasks, monitoring
   and evaluating performance and behaviour.
5. **Essential private and public services**: eligibility for public
   assistance, creditworthiness and credit scoring, risk assessment and
   pricing in life and health insurance, classifying emergency calls and
   dispatching emergency services.
6. **Law enforcement**: risk assessment of persons, polygraphs,
   evaluating evidence, profiling.
7. **Migration, asylum and border control**: risk assessment, examining
   applications, identifying persons.
8. **Administration of justice and democratic processes**: assisting
   judicial decisions, influencing elections and referendums.

### 2.3 Further excluded uses

- **Cloning a voice without consent.** Voice cloning is meant for the
  user's own voice or that of a person who has explicitly consented. A
  cloned voice must not be passed off as a genuine recording of a
  person.
- **Removing the marking of synthetic content** in order to pass it off
  as genuine.
- Everything listed in the project's exclusion catalogue
  ([`../ethics/Ausschluss.json`](../ethics/Ausschluss.json)).

## 3. Technical measures against misuse

| Measure | Effect |
|---|---|
| Notice at start-up, to be actively acknowledged | Nobody uses Myelith without knowing that it is an AI and what it is not meant for |
| Permanent "AI" label in the interface | Every answer is recognisable as AI-generated |
| Request filter | Requests clearly aimed at a prohibited practice are refused with a reference to this policy; the refusal is logged without plain text |
| Marking of the synthetic voice | Metadata and a watermark in every generated audio file |
| Confirmation before actions | By default, the agent's writes, network access and commands are presented for approval before they run |
| Emergency stop | A button and a keyboard shortcut abort any running agent action |
| Action log | Every agent action is logged locally, without plain text, for 30 days |
| No identification of persons when seeing | The vision model is instructed not to identify persons and not to attribute emotions or sensitive characteristics |

The implementation status of each measure is given in the
[self-assessment](Self-Assessment.md).

⚠️ **Limits.** A filter does not reliably recognise intent, and a program
running locally can be modified. These measures make misuse harder; they
do not prevent it. Whoever uses Myelith contrary to this policy is
responsible for that use.

## 4. Reporting

Anyone who notices a use contrary to this policy or finds a gap in the
safeguards reports it via the repository's issues.
