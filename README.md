# The Pastor Bible

A free, offline, nondenominational Bible study tool with cited answers.

## Disclaimer

> The Pastor Bible is a study and educational tool. It searches the text of the Bible and summarizes what it finds, with citations. It is not a pastor, a counselor, or an authority, and its answers are not the final word on anything. Read the verses it cites for yourself. If you are struggling, please reach out to a real person.

## If you are in crisis

> If you are in crisis, or thinking about harming yourself or someone else, please reach out to a real person right now. In the United States, call or text 988. In other countries, call your local emergency number. Talk to a pastor, a counselor, or someone you trust. This app is a study tool and cannot help you the way a person can.

## What it is, and what it is not

> The Pastor Bible is nondenominational. It uses one public-domain translation and reports what the text says and where. It includes no commentary and takes no position on questions where Christian traditions differ. The optional Deuterocanon setting is provided because some traditions include those books and others do not; it is off by default and every passage from it is labeled.

## Install

The Pastor Bible is not released yet. Installers for Windows and Linux come with
v1.0.0.

When you install on Windows, expect this:

> Because this is a free project made by volunteers, the installer is not yet signed with a paid certificate. When you run it, Windows will show a blue screen that says "Windows protected your PC." This is expected. Click "More info," then "Run anyway." (Screenshots follow.) We will apply for free open-source signing after the first release; when granted, the publisher shown will be "SignPath Foundation."

Install steps, the Linux package, and upgrading: not yet available; filled in at P6.

## First run

Not yet available; filled in at P5.

## Hardware requirements

Measured on an AMD Ryzen 7 5800X, 8 cores, on 2026-08-26. The Pastor Bible runs
entirely on your processor; it does not need a graphics card, and it will not
use one.

There are two versions of the answering model. The installer picks for you
based on how much memory your computer has, and you can change it in settings.

    Recommended, the larger model
      memory        16 GB
      disk          about 5.3 GB in total
      an answer     around 2 to 4 minutes on a processor like the one above

    Minimum, the smaller model
      memory        8 GB
      disk          about 2.3 GB in total
      an answer     around 1 to 2 minutes

Those disk figures are everything: the program, the Bible index that ships with
it, and the answering model that downloads once on first run. The index and the
search model together are about 630 MB of it and arrive with the installer. The
answering model is the rest, and it is the only thing downloaded afterwards.

If your computer has less than 8 GB of memory or too little free disk, The
Pastor Bible will tell you so on first run and stop, rather than install
something that will not work.

Answers take minutes rather than seconds because everything happens on your own
machine, with nothing sent anywhere. A faster processor is the thing that helps
most.

The timings above were measured on one machine. They will be checked on a clean
machine before release.

## Using it

Not yet available; filled in at P5.

## How answers are produced, and why you can trust the references

This section describes how v1.0.0 works. The mechanism is settled; the code that
implements it is written in P4.

The Pastor Bible does not answer from memory. Everything it says is built from
passages it has actually retrieved from the text of the Bible on your computer.

When you ask a question, the app searches its index for passages that address it,
by meaning and by wording, then widens the net using two public-domain study
aids: a cross-reference set and a topical index. It ranks what it finds and hands
the best passages to the model that writes the summary.

Those passages are handed over with anonymous labels, not with their references.
The model can only cite the labels. It is never given the option of writing out a
chapter and verse from memory, because it is never told what the references are.

Then every reference in the finished answer is checked against the passages that
were actually sent. Anything that does not match is rejected and the answer is
written again. If it fails a second time, the app does not show you a summary at
all. It shows you the passages it found, grouped by book, and says plainly that
it could not produce a summary.

The verse text you read in the passage panel is drawn from the Bible index
itself, never from the model's output. A fabricated reference cannot reach you.
That is not a claim about the model being careful. It is a check performed on
every answer.

## Privacy

Nothing leaves your computer.

The Pastor Bible uses the internet exactly twice: when you download the installer,
and when it downloads its language model the first time you run it. After that it
makes no outbound connection at all. There is no telemetry, no analytics, no crash
reporting, no account, and no sync.

Your questions and the answers to them are stored on your own machine. You can
search them, delete any of them, delete all of them, and export them to a text
file. They are never transmitted anywhere.

A test in the project's automated build runs the app's full question suite with
networking switched off and requires it to pass. A release that fails that test is
blocked.

## Building from source

Not yet available; filled in at P6.

## Sources and credits

Not yet available; filled in at P3.

Attribution for everything currently in this repository is recorded in
[NOTICE.md](NOTICE.md), and is kept current as each source is added.

## License

Apache-2.0. See [LICENSE](LICENSE).

Using The Pastor Bible places no obligations on you. All attribution obligations
are met by this repository.

## Authors

Jared — direction, orchestration, judgment.
Claude (Anthropic) — architecture and code, via Claude Code.

Copyright for this repository is held by Jared. Claude is credited as co-author
throughout, and cannot hold copyright.
