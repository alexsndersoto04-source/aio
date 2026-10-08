#!/usr/bin/env python3
"""Create on-disk audio-tag fixtures for the native audio_tags differential test.

The pathname fixtures are complete PCM WAV files with distinct POSIX byte
names that collapse to one lossy UTF-8 string. Companion directories contain
container metadata fixtures with large embedded PNG covers in ID3/MP3, FLAC
PICTURE, and WAV ID3 chunks, sparse metadata-boundary files for the 64 MiB cap,
and a real directory tree for the 5,000-entry library-scan budget. These
fixtures exercise metadata parsing, not audio playback/decoding.
"""
from __future__ import annotations

import os
import shutil
import struct
import sys
import zlib


def chunk(identifier: bytes, payload: bytes) -> bytes:
    if len(identifier) != 4:
        raise ValueError("RIFF chunk identifiers are four bytes")
    padding = b"\0" if len(payload) & 1 else b""
    return identifier + struct.pack("<I", len(payload)) + payload + padding


def wav(title: str) -> bytes:
    fmt = struct.pack("<HHIIHH", 1, 2, 44_100, 176_400, 4, 16)
    info = b"INFO" + chunk(b"INAM", title.encode("utf-8") + b"\0")
    body = b"WAVE" + chunk(b"fmt ", fmt) + chunk(b"LIST", info) + chunk(b"data", bytes(176_400))
    return b"RIFF" + struct.pack("<I", len(body)) + body


def scan_limit_wav() -> bytes:
    """Small, complete PCM/WAVE file used by the scan entry-budget fixture."""
    fmt = struct.pack("<HHIIHH", 1, 2, 44_100, 176_400, 4, 16)
    body = b"WAVE" + chunk(b"fmt ", fmt) + chunk(b"data", bytes(4))
    return b"RIFF" + struct.pack("<I", len(body)) + body


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    crc = zlib.crc32(kind + payload) & 0xFFFFFFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", crc)


def large_png() -> bytes:
    # A legal 1x1 RGB PNG with a 2 MiB ancillary text chunk. The large chunk
    # keeps the embedded image itself over the audio reader's 1 MiB head cap.
    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    text = b"Cover fixture\0" + b"A" * (2 * 1024 * 1024)
    scanline = zlib.compress(b"\0\x11\x22\x33")
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"tEXt", text)
        + png_chunk(b"IDAT", scanline)
        + png_chunk(b"IEND", b"")
    )


def syncsafe(value: int) -> bytes:
    if value < 0 or value > 0x0FFFFFFF:
        raise ValueError("ID3 syncsafe integer is out of range")
    return bytes(
        [
            (value >> 21) & 0x7F,
            (value >> 14) & 0x7F,
            (value >> 7) & 0x7F,
            value & 0x7F,
        ]
    )


def id3_frame(identifier: bytes, payload: bytes) -> bytes:
    return identifier + syncsafe(len(payload)) + b"\0\0" + payload


def id3_tag(image: bytes) -> bytes:
    apic = b"\x03image/png\0\x03\0" + image
    body = id3_frame(b"APIC", apic)
    return b"ID3\x04\0\0" + syncsafe(len(body)) + body


def mp3_with_cover(image: bytes) -> bytes:
    # MPEG-1 Layer III, 128 kbps, 44.1 kHz frame (417 bytes).
    frame = b"\xff\xfb\x90\0" + bytes(413)
    return id3_tag(image) + frame * 2


def flac_with_cover(image: bytes) -> bytes:
    # STREAMINFO declares one second of 44.1 kHz, stereo, 16-bit audio.
    streaminfo = bytearray(34)
    packed = (44_100 << 44) | (1 << 41) | (15 << 36) | 44_100
    streaminfo[10:18] = packed.to_bytes(8, "big")
    stream_block = b"\0" + len(streaminfo).to_bytes(3, "big") + bytes(streaminfo)

    mime = b"image/png"
    picture = (
        struct.pack(">II", 3, len(mime))
        + mime
        + struct.pack(">I", 0)  # empty description
        + struct.pack(">IIII", 1, 1, 24, 0)
        + struct.pack(">I", len(image))
        + image
    )
    picture_block = b"\x86" + len(picture).to_bytes(3, "big") + picture
    return b"fLaC" + stream_block + picture_block


def wav_with_cover(image: bytes) -> bytes:
    fmt = struct.pack("<HHIIHH", 1, 2, 44_100, 176_400, 4, 16)
    # Put samples before the ID3 metadata to verify that cover() seeks over
    # audio data instead of stopping at the data chunk or reading it whole.
    body = b"WAVE" + chunk(b"fmt ", fmt) + chunk(b"data", bytes(176_400)) + chunk(b"id3 ", id3_tag(image))
    return b"RIFF" + struct.pack("<I", len(body)) + body


def write(root: bytes, name: bytes, title: str) -> None:
    path = root + b"/" + name
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        data = memoryview(wav(title))
        while data:
            written = os.write(fd, data)
            if written <= 0:
                raise OSError("short write while creating WAV fixture")
            data = data[written:]
    finally:
        os.close(fd)


def write_sparse(path: bytes, prefix: bytes, length: int) -> None:
    write_sparse_segments(path, [(0, prefix)], length)


def write_sparse_segments(path: bytes, segments: list[tuple[int, bytes]], length: int) -> None:
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        for offset, payload in segments:
            os.lseek(fd, offset, os.SEEK_SET)
            remaining = memoryview(payload)
            while remaining:
                written = os.write(fd, remaining)
                if written <= 0:
                    raise OSError("short write while creating sparse metadata fixture")
                remaining = remaining[written:]
        os.ftruncate(fd, length)
    finally:
        os.close(fd)


def write_cover_fixtures(root: bytes) -> None:
    cover_root = os.path.dirname(root) + b"/audio-tags-cover"
    shutil.rmtree(cover_root, ignore_errors=True)
    os.makedirs(cover_root, mode=0o755)

    image = large_png()
    if len(image) <= 1024 * 1024:
        raise AssertionError("large cover fixture must exceed 1 MiB")
    fixtures = {
        b"cover.png": image,
        b"large.mp3": mp3_with_cover(image),
        b"large.flac": flac_with_cover(image),
        b"large.wav": wav_with_cover(image),
    }
    for name, data in fixtures.items():
        with open(cover_root + b"/" + name, "xb") as target:
            target.write(data)

    # Put another large cover after 80 MiB of sparse PCM. Extraction must
    # seek over this real audio chunk, not read it, and the audio offset must
    # not consume the 64 MiB metadata budget.
    audio_bytes = 80 * 1024 * 1024
    fmt_chunk = chunk(b"fmt ", struct.pack("<HHIIHH", 1, 2, 44_100, 176_400, 4, 16))
    id3_chunk = chunk(b"id3 ", id3_tag(image))
    id3_offset = 12 + len(fmt_chunk) + 8 + audio_bytes
    total_size = id3_offset + len(id3_chunk)
    wav_prefix = (
        b"RIFF"
        + struct.pack("<I", total_size - 8)
        + b"WAVE"
        + fmt_chunk
        + b"data"
        + struct.pack("<I", audio_bytes)
    )
    if len(wav_prefix) + audio_bytes != id3_offset:
        raise AssertionError("sparse WAV metadata offset does not match its chunks")
    write_sparse_segments(
        cover_root + b"/large-data.wav",
        [(0, wav_prefix), (id3_offset, id3_chunk)],
        total_size,
    )

    # Sparse metadata-only files reach the real declared boundary without
    # allocating tens of megabytes of fixture data on disk.
    limit_bytes = 64 * 1024 * 1024
    write_sparse(
        cover_root + b"/limit.mp3",
        b"ID3\x04\0\0" + syncsafe(limit_bytes),
        10 + limit_bytes,
    )

    # Even a complete APIC near the start must not bypass the declared-tag
    # limit. This catches implementations that return the bounded head's
    # partial parse before applying the shared scan budget.
    small_png = (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0))
        + png_chunk(b"IDAT", zlib.compress(b"\0\0\0\0"))
        + png_chunk(b"IEND", b"")
    )
    apic = b"\x03image/png\0\x03\0" + small_png
    apic_frame = b"APIC" + syncsafe(len(apic)) + b"\0\0" + apic
    write_sparse_segments(
        cover_root + b"/limit-early.mp3",
        [(0, b"ID3\x04\0\0" + syncsafe(limit_bytes)), (10, apic_frame)],
        10 + limit_bytes,
    )

    # FLAC's 24-bit block size tops out below 64 MiB. Three maximum-size
    # padding blocks put a PICTURE block beyond the shared scan limit.
    flac_padding_size = 0xFFFFFF
    picture_prefix = (
        struct.pack(">II", 3, 9)
        + b"image/png"
        + struct.pack(">I", 0)
        + struct.pack(">IIII", 1, 1, 24, 0)
        + struct.pack(">I", flac_padding_size - 41)
    )
    flac_limit_segments = [(0, b"fLaC\0\0\0\x22" + bytes(34))]
    flac_pos = 42
    for _ in range(3):
        flac_limit_segments.append((flac_pos, b"\x01" + flac_padding_size.to_bytes(3, "big")))
        flac_pos += 4 + flac_padding_size
    flac_limit_segments.append((flac_pos, b"\x86" + flac_padding_size.to_bytes(3, "big")))
    flac_limit_segments.append((flac_pos + 4, picture_prefix))
    write_sparse_segments(
        cover_root + b"/limit.flac",
        flac_limit_segments,
        flac_pos + 4 + flac_padding_size,
    )

    # A RIFF JUNK metadata chunk spans more than the scan budget. The reader
    # must seek over it and report the cap before attempting another chunk.
    wav_limit_prefix = (
        b"RIFF"
        + struct.pack("<I", 4 + 8 + limit_bytes)
        + b"WAVEJUNK"
        + struct.pack("<I", limit_bytes)
    )
    write_sparse(
        cover_root + b"/limit.wav",
        wav_limit_prefix,
        len(wav_limit_prefix) + limit_bytes,
    )


def write_scan_limit_fixtures(root: bytes) -> None:
    """Create more than 5,000 directory entries with valid tracks at 2 per album."""
    scan_root = os.path.dirname(root) + b"/audio-tags-scan-limit"
    shutil.rmtree(scan_root, ignore_errors=True)
    os.makedirs(scan_root, mode=0o755)
    audio = scan_limit_wav()
    # Every discovered track requires examining both its album directory entry
    # and the WAV entry inside it. The extra album must be beyond a 5,000-entry
    # budget, regardless of the filesystem's directory iteration order.
    for index in range(5000 // 2 + 1):
        album = scan_root + b"/album-" + f"{index:04d}".encode("ascii")
        os.mkdir(album, mode=0o755)
        with open(album + b"/track.wav", "xb") as target:
            target.write(audio)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: prepare_audio_tag_aliases.py DIRECTORY")
    root = os.path.abspath(os.fsencode(sys.argv[1]))
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root, mode=0o755)
    # U+FFFD in UTF-8 and a raw 0xFF byte have the same lossy display spelling.
    write(root, b"alias-\xef\xbf\xbd.wav", "valid UTF-8 filename")
    write(root, b"alias-\xff.wav", "invalid UTF-8 filename")
    write_cover_fixtures(root)
    write_scan_limit_fixtures(root)


if __name__ == "__main__":
    main()
