# Packed HAL2 playback on channel zero

NetBSD haltwo and Linux snd-hal2 program channel zero with `0x08208844`.
Their playback buffers contain consecutive signed 16-bit left and right
samples. Advancing four bytes per sample loses the right channel. Reading
only the high byte turns the sign transition into a faint square wave.
The SGI PROM uses a different configuration with one word per sample.
Exercise both formats when changing the PBUS reader.
