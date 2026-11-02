# NetBSD XAA fast-clear colors do not match image uploads

Status: RGB24 renderer correction implemented and unit-tested.
Original finding: NetBSD 11.0/sgimips, Xorg 1.10.6, R4400, Newport, baseline
b6d6f5c. Reproduced on xiri e049373 with the framed-console kernel on 2026-11-02.
Physical hardware has not been tested.

With the release `newport` driver and acceleration enabled, an Xlib red fill
allocated as pixel 0x0000fe reads back as 0x092492. A screen-to-screen copy
preserves the wrong color. A four-color XPutImage tile renders and reads back
correctly. The composed frame also has incorrect fill colors, so this is not
only an XGetImage problem.

`NewportXAASetupForSolidFill` converts GXcopy colors through
`NewportColor2Planes24` and writes COLORVRAM with FASTCLEAR set. That conversion
interleaves reversed G/R/B component bits into physical-plane order; 0xfe
becomes 0x092492. IRIS stores COLORVRAM unchanged and `fastclear_color` currently
returns its low 24 bits at depth 24. WRMASK receives the same driver conversion.
Check the register format and its dependence on DRAWDEPTH/RGBMODE against
hardware before changing the renderer or driver. Existing IRIX workloads and
other depths/planes must be considered together.

The release source is in NetBSD xsrc, separate from the src/kernel repository:
https://github.com/NetBSD/xsrc/blob/netbsd-11/external/mit/xf86-video-newport/dist/src/newport_accel.c

`Option "NoAccel" "true"` uses the shadow framebuffer and avoids this path.
Xiri's `lab/indy-netbsd/x11/probe.c` checks fill, copy, XOR, and image upload,
with separate checks of the emulator's composed pixels. The exploration
record is in Xiri `.work/netbsd-x/explore-2ybly29u/`; `probe.png` shows the
accelerated failure and `noaccel.png` the corrected color swatches.

Geometry is also not fully qualified: this run's X screen is 1024x768 while
`screen info` reports 1282x1024. The unused region is black. The serial-console
kernel logs a rasops stride warning when allocating wsdisplay screen 0.

## Hardware format and correction (2026-11-02)

The user's `../sgi/sources.json` catalog led to the
[RB2 specification, revision 2.3](https://erikarn.github.io/sgi/indy/datasheets/sgi_indy_rb2.pdf).
Section 4.2 describes direct write-mask delivery and fast-clear bypass of the
write formatter. Table 4.4 agrees with REX3 section 3.9 table 22: RGB24 physical
bits 23..0 are B0 R0 G0 ... B7 R7 G7. NetBSD's plane conversion follows that
layout. REX3 COLORI/HOSTRW still carry logical BGR components.

IRIS uses packed BGR framebuffer words. Decode COLORVRAM at the fast-clear
boundary and WRMASK for every RGB24 write. Keep the MMIO/context/snapshot
values raw, so register readback does not change. Both the generic renderer
(and its generated LLVM specializations) and the Cranelift block/line
prologues perform the conversion. The latter hoists it out of the pixel loop.
The correction is restricted to RGB planes, RGBMODE, DRAWDEPTH=24; other
pixel-depth and auxiliary-plane layouts remain a separate audit.

The new register-driven regression has an independent table of all 24 physical
bit positions, primary-color values, channel/zero/full/single-bit masks, and
known destination pixels. It checks fast-clear block/span draws, ordinary
block/line draws, XOR and screen copies against expected framebuffer values;
register readback and auxiliary pixels must remain unchanged. It forces and
counts compiled dispatch when testing Cranelift. Both implementations failed
before the correction on the first partial red mask (0x492492).

Validation: 183 Newport tests pass with `--features rex-jit` (5 benchmark
tests ignored), including 1,044 expected-value draws in each of the interpreter
and compiled regressions. Guest XAA and software controls are tracked in the
Xiri framed-console/acceleration lab; physical hardware remains untested.
