rhex
====

A vewy simple (but functional) terminal-based hex viewer.

```
    00000000  89 50 4E 47 0D 0A 1A 0A  00 00 00 0D 49 48 44 52  .PNG........IHDR
    00000010  00 00 00 80 00 00 00 40  08 06 00 00 00 D2 D6 7F  .......@........
    00000020  7F 00 00 01 85 69 43 43  50 49 43 43 20 70 72 6F  .....iCCPICC pro
    00000030  66 69 6C 65 00 00 28 91  7D 91 3D 48 C3 50 14 85  file..(.}.=H.P..
    00000040  4F 53 A5 22 15 C1 76 10  51 08 58 9D 2C 88 8A 38  OS."..v.Q.X.,..8
    00000050  6A 15 8A 50 21 D4 0A AD  3A 98 BC F4 0F 9A 34 24  j..P!...:.....4$
    00000060  2D 2E 8E 82 6B C1 C1 9F  C5 AA 83 8B B3 AE 0E AE  -...k...........
    00000070  82 20 F8 03 E2 E8 E4 A4  E8 22 25 DE 97 14 5A C4  . ......."%...Z.
    00000080  78 E1 F1 3E CE BB E7 F0  DE 7D 80 50 2F 31 CD EA  x..>.....}.P/1..
    00000090  18 07 34 BD 62 26 E3 31  31 9D 59 15 03 AF F0 61  ..4.b&.11.Y....a
    000000A0  08 21 0C A3 4F 66 96 31  27 49 09 78 D6 D7 3D 75  .!..Of.1'I.x..=u
    000000B0  53 DD 45 79 96 77 DF 9F  D5 A3 66 2D 06 F8 44 E2  S.Ey.w....f-..D.
    000000C0  59 66 98 15 E2 0D E2 E9  CD 8A C1 79 9F 38 CC 0A  Yf.........y.8..
    000000D0  B2 4A 7C 4E 3C 66 D2 05  89 1F B9 AE B8 FC C6 39  .J|N<f.........9
    000000E0  EF B0 C0 33 C3 66 2A 39  4F 1C 26 16 F3 6D AC B4  ...3.f*9O.&..m..
    000000F0  31 2B 98 1A F1 14 71 44  D5 74 CA 17 D2 2E AB 9C  1+....qD.t......
    00000100  B7 38 6B A5 2A 6B DE 93  BF 30 98 D5 57 96 B9 4E  .8k.*k...0..W..N
    00000110  6B 10 71 2C 62 09 12 44  28 A8 A2 88 12 2A 88 D2  k.q,b..D(....*..
    00000120  AE 93 62 21 49 E7 31 0F  FF 80 E3 97 C8 A5 90 AB  ..b!I.1.........
    @0x00000000                @0                         size: 3199
    i8 : -119                  u8 : 137                   0x89
    i16: 20617                 u16: 20617                 0x5089
    i32: 1196314761            u32: 1196314761            0x474e5089
    i64: 727905341920923785    u64: 727905341920923785    0x0a1a0a0d474e5089
    f32: 52816.53515625000000  f64: 5.2923977765726e-260  little endian
```

Keys:

 * use arrows to navigate
 * pageup/pagedown, home/end should also work
 * press 'e' to toggle endianess
 * press 'l' for little endian
 * press 'b' for big endian
 * press 'q' or Esc to exit
