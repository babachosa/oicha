# Bundled Literata

Unmodified static text faces from https://github.com/googlefonts/literata,
pinned commit `0c2761b727a1b3a7cffd313c37f0f5163dfc7a63`, `fonts/ttf/`.
Copyright 2017 The Literata Project Authors. Licensed under SIL OFL 1.1;
the original license is included as OFL.txt and shipped with the application.

| File | SHA-256 |
| --- | --- |
| Literata-Regular.ttf | 0390890de9bb9d5862a6ba4125b82c61792ccc3d66b63e73eee75c1a16fcd208 |
| Literata-Italic.ttf | 198f70cc9a17bab578553fa274b81984d58c440efe26bc06f1d841c194b6691a |
| Literata-Bold.ttf | b6af95b3b443cdbce964aa06741596987f2f5c3ede46a2bc846e5addd99d061f |
| Literata-BoldItalic.ttf | 1dade59381ad02f4679d7e993a7a406472e587ac332af7a6168d3ad9214a99dc |

All four faces are embedded in the executable and registered with GPUI before
the first window is created. No network or OS font installation is required.
Source fonts cover the Vietnamese precomposed letters and combining marks;
the native smoke suite also verifies Vietnamese shaping without fallback at
16/20/28 px for regular, italic, bold and bold italic.
