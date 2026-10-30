# NetBSD XAA fast-clear colors do not match image uploads

Status: open. NetBSD 11.0/sgimips, Xorg 1.10.6, R4400, Newport, IRIS baseline
b6d6f5c. This is a driver/emulator interoperability finding; physical hardware
has not established which side needs correction.

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
