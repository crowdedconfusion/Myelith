# GolemOS: Myelith from a USB stick

**Deutsch:** [ANLEITUNG.md](ANLEITUNG.md)

GolemOS is a small operating system with a single job: running
Myelith. It fits on a USB stick. You can use Myelith straight from the
stick without changing anything on your computer, or install GolemOS
permanently on a disk later.

This guide assumes no prior knowledge. Allow about half an hour the
first time.

> The GolemOS assistant asks for your language first (German or
> English) and passes it on to Myelith. The menu numbers below are the
> same in both languages.

---

## What you need

| | |
|---|---|
| **A USB stick** | at least 4 GB for the smallest model, 16 GB or more is better. ⚠️ Everything on it will be erased |
| **A computer to run GolemOS on** | almost any PC or laptop with an Intel or AMD processor, including Macs with an Intel processor. For Macs with Apple chips see [below](#macs-with-apple-chips) |
| **Your Myelith folder** | the folder that holds your models (`INTEGER_LLM/artifacts/`) |
| **A second computer** | to write the stick and copy the folder onto it. It can be the same one |
| **A network cable** | only if Myelith should search the web. GolemOS does not support Wi-Fi yet |

**Which image?** In the Myelith folder under `SYSTEM/golemos/abbild/fertig/`:

- `golemos-x86_64.img.xz` for almost all PCs and laptops and for Macs
  with an Intel processor. **If unsure, use this one.**
- `golemos-aarch64.img.xz` for ARM computers with UEFI, such as some
  servers.

---

## Step 1: Write the image to the stick

The easiest way is **balenaEtcher** (free, for Windows, macOS and Linux,
from `etcher.balena.io`):

1. Open Etcher, click **Flash from file** and choose
   `golemos-x86_64.img.xz`. There is no need to unpack it.
2. **Select target**: choose your USB stick. Check the size so you do
   not pick another disk by mistake.
3. Click **Flash!** and wait until Etcher is done.

On Windows, **Rufus** (`rufus.ie`) works too: choose the stick, choose
the image under "Boot selection", click **Start**.

If you use a terminal (macOS, Linux), run `sh SYSTEM/golemos/spread.sh`
from the Myelith folder. Without arguments it only lists suitable
drives, and it writes only after you type the stick's name.

> After writing, your computer may call the stick "unreadable" or offer
> to format it. **Do not format it.** This is normal; the readable part
> is created on the first start.

---

## Step 2: Start from the stick

The computer has to start from the stick once instead of its own disk.
Almost every computer has a **boot menu** that opens with a key right
after switching on:

| Manufacturer | Boot menu key |
|---|---|
| Dell, Lenovo, Acer, Toshiba | F12 |
| HP | F9 (or Esc, then F9) |
| ASUS | F8 or Esc |
| MSI, ASRock, Gigabyte | F11 or F12 |
| Mac with an Intel processor | hold ⌥ (Option), then "EFI Boot" |

1. Plug in the stick, switch the computer off.
2. Switch it on and immediately press the key several times.
3. Choose the USB stick in the menu (often "UEFI: …" with its name).

### ⚠️ If the stick does not start: Secure Boot

Many computers only start systems signed by Microsoft. GolemOS is not
signed, so you need to **turn off Secure Boot**:

1. While switching on, press the key for the settings, usually **F2**
   or **Del**.
2. Under "Security" or "Boot", set **Secure Boot** to **Disabled**.
3. Save (usually F10) and restart.

On Intel Macs with a T2 chip: in recovery mode (⌘R at startup) open
"Startup Security Utility" and choose **No Security** and **Allow
booting from external media**.

Windows or macOS keep starting normally afterwards; you can turn Secure
Boot back on at any time.

---

## Step 3: The first start

1. A menu with **GolemOS (A)** appears and starts by itself after five
   seconds.
2. After a few seconds you see `golemos login:`. Type **root** and
   press Enter. There is no password.
3. The **assistant** first asks for the **language** (1 Deutsch,
   2 English) and then for your **keyboard** (1 German, 2 Swiss, 3 US
   English …). Enter takes the default. The stick remembers both; you
   can change them later with item **7**.
4. It then shows what it found and marks the sensible next step with
   `<- recommended`.

⚠️ **Until you choose the keyboard, it uses the US layout**; you only
need digits and Enter for that, which sit in the same place almost
everywhere. On a French keyboard type the digits with Shift.

On the first start there is no model yet. Choose **2** (bring the
Myelith folder onto this stick). The assistant explains the next step
and switches the computer off if you like.

---

## Step 4: Bring your Myelith folder onto the stick

1. Unplug the stick and plug it into your normal computer. It now shows
   up as **Golem**, with a file `README.txt` in it.
2. Drag your **whole Myelith folder** into **Golem**, including the
   models under `INTEGER_LLM/artifacts/`. The whole folder, because
   GolemOS also writes new sticks from it and should carry everything
   Myelith is made of. It is small; the models are the large part.

   You may leave out only what a freshly downloaded folder does not
   contain: `SYSTEM/full-build` and `SYSTEM/crates-lager` (created when
   building), the raw weights under `MODELS` (models are built from
   them; GolemOS needs the finished ones) and folders named `.venv`.
   Those contain links the stick cannot store, and copying would stop
   there.
3. **Eject** the stick safely.

**Which model fits?** GolemOS picks the largest one that fits into
about six tenths of the memory:

| Model | Size | Memory from |
|---|---|---|
| `myelith-0.6b` | 0.9 GB | 2 GB |
| `myelith-4b` | 4.5 GB | 8 GB |
| `myelith-8b` | 8.8 GB | 16 GB |

> The folder may also be on a **second stick or an external disk**, at
> most two folders deep. GolemOS searches all drives at startup. The
> file system has to be FAT32 or ext4; GolemOS cannot read exFAT or NTFS
> yet.

---

## Step 5: Use Myelith without installing anything

1. Start from the stick again (step 2) and log in as **root**.
2. The assistant shows your folder and model. Choose **1** (use Myelith
   now, without installing anything).
3. Myelith tells you that you are working with an AI and, the first
   time, shows a few rules for its tools. Read them and press Enter
   each time.
4. Myelith asks for a **design** and the **model**. Choose with the
   arrow keys and press Enter; the preselected entries fit, so pressing
   Enter twice is enough.
5. The model loads for a few seconds. Now you can type. Quit with
   **/ende** and Enter; you are then back in the assistant.

The assistant shows whether your folder is **complete**. If it says
"UNVOLLSTAENDIG" (incomplete), it names what is missing; copy the whole
folder again.

**What works:** the conversation, the agent with its tools (it works in
the folder `arbeit` on the stick), the knowledge folders and, with a
network cable, web search. Everything you set up and produce stays on
the stick. Your computer's disk is not touched.

**What does not work:** the window with the mouse, speech output and
vision. Those need Myelith on your normal system. GolemOS does not
support Wi-Fi yet.

You can keep using GolemOS this way; installing is not required.

---

## Step 6 (optional): Install GolemOS permanently

Worth it if a computer should run **only** GolemOS. A disk is faster
than a stick, and the stick becomes free again.

⛔️ **The chosen disk is erased completely**, including any Windows on
it. Back up anything you want to keep first.

1. Start from the stick, choose **3** in the assistant.
2. The assistant lists all disks. Only suitable ones get a number; the
   others show why not (for example "GolemOS is running from this disk"
   for the stick).
3. Enter the disk's number. The assistant shows what it will do and
   what will be erased.
4. **Your Myelith folder always comes along**, so the installed GolemOS
   has it. The assistant only asks whether your other data on the stick
   should come too (settings, work folder). Enter means yes.
5. **Confirm by typing the disk's name** as shown in brackets, such as
   `sda` or `nvme0n1`. A "yes" is deliberately not enough.
6. Wait until "GolemOS ist auf … installiert" appears. Switch off,
   **unplug the stick** and switch on.

The stick stays a complete GolemOS; keep using it or take it to a
second computer.

---

## Write another GolemOS stick

GolemOS can pass itself on, from the stick as well as from an installed
system, because your Myelith folder carries the images and the tool
for it.

1. Choose **4** in the assistant.
2. Plug in the new stick and press Enter. ⚠️ Everything on it will be
   erased.
3. The assistant lists the drives. Enter the new stick's, such as
   `/dev/sdb`.
4. Type the name again to confirm. Writing takes a few minutes.

The stick GolemOS is running from and every disk that is not removable
are refused. The new stick is a fresh GolemOS; afterwards drag your
Myelith folder onto it as in step 4.

---

## If something goes wrong

| What happens | What helps |
|---|---|
| The computer just starts Windows or macOS | The boot menu was missed: press the key several times right after switching on (step 2) |
| The stick is missing from the boot menu, or "Security Violation" appears | Turn off Secure Boot (step 2) |
| "kein Klon gefunden" (no folder found) | The folder is too deep or lacks `INTEGER_LLM/scripts/build_artifacts.sh`. Choose **5** in the assistant to search again |
| "UNVOLLSTAENDIG" (incomplete) | Only part of the folder was copied. Copy the whole Myelith folder (step 4) |
| Copying onto the stick stops | Usually at a `.venv` folder or at `SYSTEM/full-build`; leave those out (step 4) |
| "Noch ist kein Modell da" (no model yet) | The folder lacks `INTEGER_LLM/artifacts/<model>` |
| Myelith answers very slowly | The model is too large for the memory. Put a smaller model into the folder |
| "Golem" does not show up on your computer | Start the stick once in GolemOS; the first start creates this part |
| Typing gives the wrong characters | Choose the right keyboard with item **7** in the assistant |
| The assistant no longer appears by itself | Intended after choosing "Zur Konsole". Start it with `golem-einrichten` |

---

## Macs with Apple chips

GolemOS does **not yet start from a stick** on Macs with Apple chips:
these Macs have no UEFI of the kind GolemOS needs.

The planned route is **Asahi Linux**: its installer can set up just a
small UEFI environment (about 3 GB) which then boots any UEFI USB stick
by itself, GolemOS included. GolemOS still lacks a kernel with drivers
for Apple's hardware, and Asahi itself supports Macs with M1, M2 and M3
so far; M4 and M5 are in progress.

---

## Good to know

- **There is no password.** Whoever sits at the keyboard can do
  anything. GolemOS opens no access over the network.
- **Myelith tells you at every start** that you are working with an AI,
  as required by the EU AI Act.
- **Self-test:** in the boot menu, **GolemOS Selbstprobe** starts the
  system, checks whether Myelith answers and switches off again.
