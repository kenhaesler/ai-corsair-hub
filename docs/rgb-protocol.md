# iCUE LINK RGB Protocol

> Protocol sourced from [OpenLinkHub](https://github.com/jurkovic-nikola/OpenLinkHub) by Nikola Jurkovic.
> Ported to Rust for ai-corsair-hub.

## Overview

The iCUE LINK System Hub (VID `0x1B1C`, PID `0x0C3F`) uses a **separate color endpoint** for RGB data, distinct from the data endpoint used for fan/temp/device operations.

- **Data endpoint**: `0x0D 0x01` (open), `0x06 0x01` (write), `0x05 0x01 0x01` (close)
- **Color endpoint**: `0x0D 0x00` (open), `0x06 0x00` (write), `0x05 0x01` (close)

## Command Reference

| Command | Bytes | Description |
|---------|-------|-------------|
| Open color EP | `0x0D, 0x00` + mode `0x22` | Opens the color endpoint |
| Close color EP | `0x05, 0x01` | Closes the color endpoint (shorter than data EP close) |
| Write color (first chunk) | `0x06, 0x00` + payload | First chunk of color data |
| Write color (continuation) | `0x07, 0x00` + payload | Subsequent chunks when data > 508 bytes |
| Close data EP (stale) | `0x05, 0x01, 0x01` | Close stale data EP before opening color EP |
| Reset LED power | `0x15, 0x01` | Reset LED power state |

## Write Sequence

```
1. Close any stale data endpoint:
   TX: [0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x00, ...]  (513 bytes)
   RX: 512 bytes

2. Open color endpoint with mode 0x22:
   TX: [0x00, 0x00, 0x01, 0x0D, 0x00, 0x22, 0x00, ...]  (513 bytes)
   RX: 512 bytes

3. Send color data (repeat at 20ms+ intervals):
   First chunk:
   TX: [0x00, 0x00, 0x01, 0x06, 0x00, <payload[0..508]>]  (513 bytes)
   RX: 512 bytes

   Continuation chunks (if payload > 508 bytes):
   TX: [0x00, 0x00, 0x01, 0x07, 0x00, <payload[508..1016]>]  (513 bytes)
   RX: 512 bytes
```

## Payload Format

All RGB data for ALL devices on the hub is sent in one flat buffer, concatenated by ascending channel order:

```
Offset  Size   Field
------  ----   -----
0       2      uint16_le(total_payload_length)  // includes these 2 bytes
2       2      reserved (0x00, 0x00)
4       2      dataTypeSetColor (0x12, 0x00)
6       N*3    RGB data: [R, G, B, R, G, B, ...]
```

- **Byte order**: RGB (not GRB)
- **Bytes per LED**: 3
- **Max per USB transfer**: 508 bytes (after the command prefix)
- **Chunking**: If total payload > 508 bytes, split and send continuation chunks with command `[0x07, 0x00]`

## LED Counts by Device Type

| Device | Type Byte | LEDs | Name |
|--------|-----------|------|------|
| QX Fan | `0x01` | 34 | iCUE LINK QX RGB |
| LX Fan | `0x02` | 18 | iCUE LINK LX RGB |
| RX MAX RGB | `0x03` | 8 | iCUE LINK RX RGB MAX |
| RX MAX | `0x04` | 0 | No RGB |
| LINK Adapter | `0x05` | dynamic | LED strip host |
| Liquid Cooler | `0x07` | 20 | H100i–H170i |
| Water Block | `0x09` | 24 | XC7 ELITE |
| GPU Block | `0x0A` | 22 | XG3 HYBRID |
| PSU | `0x0B` | 0 | No RGB |
| XD5 Pump | `0x0C` | 22 | XD5 ELITE |
| XG7 Block | `0x0D` | 16 | XG7 RGB |
| RX RGB Fan | `0x0F` | 8 | iCUE LINK RX RGB |
| Titan Cooler | `0x11` | 20 | TITAN 240–420 |
| XD6 Pump | `0x19` | 22 | XD6 ELITE |

## Port Power Protection

When too many LEDs are connected to one hub port, brightness must be limited to prevent USB power issues:

| Total LEDs | Condition | Brightness Factor |
|------------|-----------|------------------|
| 0–238 | ≤7 QX fans | 1.00 (full) |
| 239–340 | 7–10 QX fans | 0.66 |
| 341–442 | 10–13 QX fans | 0.33 |
| 443+ | 13+ QX fans | 0.10 |

The brightness factor is applied as a multiplier to all RGB values before sending to hardware.

## Endpoint Lifecycle

1. **Open once**: Call `open_color_endpoint()` when starting RGB rendering
2. **Write repeatedly**: Call `write_color()` at 20ms+ intervals (30–60 FPS)
3. **Close on shutdown**: Call `close_color_endpoint()` or `enter_hardware_mode()` — the latter closes the color EP automatically before sending the sleep command

The color endpoint stays open for the duration of RGB rendering. It is automatically closed when switching back to hardware mode.

## Hex Example

Setting 2 QX fans (34 LEDs each = 68 LEDs = 204 bytes RGB data) to solid red:

```
Payload layout:
  [D2, 00]           // length = 210 (2+2+2+204)
  [00, 00]           // reserved
  [12, 00]           // dataTypeSetColor
  [FF, 00, 00] × 68  // 68 LEDs, all red

Total payload: 210 bytes (fits in one 508-byte chunk)

USB transfer:
  TX: [00, 00, 01, 06, 00, D2, 00, 00, 00, 12, 00, FF, 00, 00, FF, 00, 00, ...]
```

## References

- [OpenLinkHub source](https://github.com/jurkovic-nikola/OpenLinkHub) — Go implementation by Nikola Jurkovic
- [liquidctl](https://github.com/liquidctl/liquidctl) — Python, covers HX1500i PSU but not iCUE LINK RGB
