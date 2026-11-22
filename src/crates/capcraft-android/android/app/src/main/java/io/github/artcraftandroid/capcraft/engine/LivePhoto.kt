package io.github.artcraftandroid.capcraft.engine

import android.content.Context
import android.graphics.Bitmap
import android.media.Image
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.MediaMuxer
import io.github.artcraftandroid.capcraft.core.MobileSession
import io.github.artcraftandroid.capcraft.core.Motion
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.RandomAccessFile

/**
 * Live photos (motion photos, as Android phones keep them): a picture with a short video after it in
 * the same file. One's video is taken out to edit, and the video round the playhead is put behind its
 * frame to make one.
 */
object LivePhoto {
    /** How much of the video goes in a live photo, either side of its picture, microseconds. */
    private const val AROUND_US = 1_200_000L
    private const val FPS = 30

    /** Where the video in picture [file] begins and how long it is, or null when there is none. */
    fun videoIn(file: File): Pair<Long, Long>? = RandomAccessFile(file, "r").use { f ->
        val length = f.length()
        if (length < 64 || length > (512L shl 20)) return null
        // a video begins with a box named ftyp; a HEIC picture begins with one too, so the file's first is passed over
        val name = byteArrayOf('f'.code.toByte(), 't'.code.toByte(), 'y'.code.toByte(), 'p'.code.toByte())
        val chunk = ByteArray(1 shl 16)
        var at = 16L
        while (at < length - 16) {
            f.seek(at)
            val n = f.read(chunk)
            if (n < 8) break
            for (i in 0..n - 4) {
                if (chunk[i] == name[0] && chunk[i + 1] == name[1] && chunk[i + 2] == name[2] && chunk[i + 3] == name[3]) {
                    val start = at + i - 4
                    val end = boxes(f, start, length)
                    if (end != null) return start to end - start
                }
            }
            at += n - 3
        }
        null
    }

    /** Where the boxes from [start] end, or null when they are no video: it has its samples and its index. */
    private fun boxes(f: RandomAccessFile, start: Long, length: Long): Long? {
        var at = start
        var index = false
        var samples = false
        while (at + 8 <= length) {
            f.seek(at)
            var size = f.readInt().toLong() and 0xFFFFFFFFL
            val type = ByteArray(4).also { f.readFully(it) }.let { String(it, Charsets.ISO_8859_1) }
            if (size == 1L) size = f.readLong()
            if (size == 0L) size = length - at
            if (size < 8 || at + size > length || type.any { it !in ' '..'~' }) break
            if (type == "moov") index = true
            if (type == "mdat") samples = true
            at += size
        }
        return if (index && samples) at else null
    }

    /** The video in picture [file], copied beside it as a file of its own; null when there is none. */
    fun takeOut(file: File): File? {
        val (start, size) = videoIn(file) ?: return null
        val out = Media.uniqueFile(file.parentFile ?: return null, file.nameWithoutExtension + " live.mp4")
        RandomAccessFile(file, "r").use { f ->
            f.seek(start)
            out.outputStream().use { o ->
                val buffer = ByteArray(1 shl 16)
                var left = size
                while (left > 0) {
                    val n = f.read(buffer, 0, minOf(left, buffer.size.toLong()).toInt())
                    if (n < 0) break
                    o.write(buffer, 0, n)
                    left -= n
                }
            }
        }
        return out
    }

    /** A live photo of [session]'s timeline at [atUs], [w] by [h], its video round it; [progress] gets 0 to 1. */
    fun make(context: Context, session: MobileSession, atUs: Long, durationUs: Long, w: Int, h: Int, progress: (Float) -> Unit): ByteArray {
        val from = (atUs - AROUND_US).coerceAtLeast(0)
        val to = (atUs + AROUND_US).coerceAtMost(durationUs)
        val video = File(context.cacheDir, "live-" + System.nanoTime() + ".mp4")
        try {
            // the video at a size any phone's encoder takes
            val scale = minOf(1.0, 1080.0 / minOf(w, h))
            film(session, from, to, (w * scale).toInt() / 2 * 2, (h * scale).toInt() / 2 * 2, video, progress)
            val picture = session.frame(atUs, w.toUInt(), h.toUInt(), Motion.STILL).intoBitmap(null) ?: error("The picture could not be made.")
            val jpeg = ByteArrayOutputStream().also { picture.compress(Bitmap.CompressFormat.JPEG, 95, it) }.toByteArray()
            val clip = video.readBytes()
            return withVideo(jpeg, clip, atUs - from)
        } finally {
            video.delete()
        }
    }

    /** [jpeg] with [video] behind it and the note in it that says so. */
    internal fun withVideo(jpeg: ByteArray, video: ByteArray, shownUs: Long): ByteArray {
        val xmp = """<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:GCamera="http://ns.google.com/photos/1.0/camera/" xmlns:Container="http://ns.google.com/photos/1.0/container/" xmlns:Item="http://ns.google.com/photos/1.0/container/item/" GCamera:MotionPhoto="1" GCamera:MotionPhotoVersion="1" GCamera:MotionPhotoPresentationTimestampUs="$shownUs" GCamera:MicroVideo="1" GCamera:MicroVideoVersion="1" GCamera:MicroVideoOffset="${video.size}" GCamera:MicroVideoPresentationTimestampUs="$shownUs"><Container:Directory><rdf:Seq><rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="image/jpeg" Item:Semantic="Primary" Item:Length="0" Item:Padding="0"/></rdf:li><rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="video/mp4" Item:Semantic="MotionPhoto" Item:Length="${video.size}" Item:Padding="0"/></rdf:li></rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>"""
        val body = "http://ns.adobe.com/xap/1.0/\u0000".toByteArray(Charsets.ISO_8859_1) + xmp.toByteArray()
        val out = ByteArrayOutputStream(jpeg.size + video.size + body.size + 4)
        // the note goes right after the picture's first mark
        out.write(jpeg, 0, 2)
        out.write(0xFF)
        out.write(0xE1)
        out.write((body.size + 2) shr 8)
        out.write((body.size + 2) and 0xFF)
        out.write(body)
        out.write(jpeg, 2, jpeg.size - 2)
        out.write(video)
        return out.toByteArray()
    }

    /** The timeline from [from] to [to] as a silent H.264 video. */
    private fun film(session: MobileSession, from: Long, to: Long, w: Int, h: Int, out: File, progress: (Float) -> Unit) {
        val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, w, h).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible)
            setInteger(MediaFormat.KEY_BIT_RATE, (w * h * FPS * 0.2).toInt().coerceIn(2_000_000, 20_000_000))
            setInteger(MediaFormat.KEY_FRAME_RATE, FPS)
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 1)
            setInteger(MediaFormat.KEY_COLOR_STANDARD, MediaFormat.COLOR_STANDARD_BT709)
            setInteger(MediaFormat.KEY_COLOR_RANGE, MediaFormat.COLOR_RANGE_LIMITED)
            setInteger(MediaFormat.KEY_COLOR_TRANSFER, MediaFormat.COLOR_TRANSFER_SDR_VIDEO)
        }
        val codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        val muxer = MediaMuxer(out.absolutePath, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4)
        var track = -1
        try {
            codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            codec.start()
            val count = ((to - from) * FPS / 1_000_000L).toInt().coerceAtLeast(1)
            val info = MediaCodec.BufferInfo()
            var sent = 0
            var ended = false
            while (true) {
                if (!ended) {
                    val index = codec.dequeueInputBuffer(10_000)
                    if (index >= 0) {
                        val pts = sent * 1_000_000L / FPS
                        if (sent >= count) {
                            codec.queueInputBuffer(index, 0, 0, pts, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                            ended = true
                        } else {
                            val rgba = session.frame(from + pts, w.toUInt(), h.toUInt(), Motion.STILL).rgba
                            val image = codec.getInputImage(index) ?: error("The encoder gave no room for a frame.")
                            fill(image, rgba, w, h)
                            codec.queueInputBuffer(index, 0, w * h * 3 / 2, pts, 0)
                            sent++
                            progress(sent.toFloat() / count)
                        }
                    }
                }
                val index = codec.dequeueOutputBuffer(info, 10_000)
                if (index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                    track = muxer.addTrack(codec.outputFormat)
                    muxer.start()
                } else if (index >= 0) {
                    val buffer = codec.getOutputBuffer(index)
                    if (buffer != null && info.size > 0 && info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG == 0 && track >= 0) {
                        muxer.writeSampleData(track, buffer, info)
                    }
                    codec.releaseOutputBuffer(index, false)
                    if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) break
                }
            }
        } finally {
            runCatching { codec.stop() }
            codec.release()
            runCatching { if (track >= 0) muxer.stop() }
            runCatching { muxer.release() }
        }
    }

    /** Writes RGBA as the encoder's Y, U and V (BT.709, limited range), whatever way its planes are laid out. */
    private fun fill(image: Image, rgba: ByteArray, w: Int, h: Int) {
        val (yp, up, vp) = image.planes
        val yb = yp.buffer
        for (y in 0 until h) {
            var i = y * w * 4
            var at = y * yp.rowStride
            for (x in 0 until w) {
                val r = rgba[i].toInt() and 255
                val g = rgba[i + 1].toInt() and 255
                val b = rgba[i + 2].toInt() and 255
                yb.put(at, (16 + ((47 * r + 157 * g + 16 * b + 128) shr 8)).toByte())
                i += 4
                at += yp.pixelStride
            }
        }
        val ub = up.buffer
        val vb = vp.buffer
        for (y in 0 until h / 2) {
            for (x in 0 until w / 2) {
                // the four pixels this one colour is for
                var r = 0
                var g = 0
                var b = 0
                for (dy in 0..1) for (dx in 0..1) {
                    val i = ((y * 2 + dy) * w + x * 2 + dx) * 4
                    r += rgba[i].toInt() and 255
                    g += rgba[i + 1].toInt() and 255
                    b += rgba[i + 2].toInt() and 255
                }
                r /= 4
                g /= 4
                b /= 4
                ub.put(y * up.rowStride + x * up.pixelStride, (128 + ((-26 * r - 87 * g + 112 * b + 128) shr 8)).toByte())
                vb.put(y * vp.rowStride + x * vp.pixelStride, (128 + ((112 * r - 102 * g - 10 * b + 128) shr 8)).toByte())
            }
        }
    }
}
