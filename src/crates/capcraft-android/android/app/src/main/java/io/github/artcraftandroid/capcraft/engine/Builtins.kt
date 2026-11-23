package io.github.artcraftandroid.capcraft.engine

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import java.io.File
import java.io.FileOutputStream

/** What a built-in template needs drawn before it is laid out: a picture for each place, and its graphics. */
object Builtins {
    private val HUES = listOf(
        0xFF3A1C71.toInt() to 0xFFD76D77.toInt(),
        0xFF0F2027.toInt() to 0xFF2C5364.toInt(),
        0xFF614385.toInt() to 0xFF516395.toInt(),
        0xFF1D2B64.toInt() to 0xFFF8CDDA.toInt(),
        0xFF134E5E.toInt() to 0xFF71B280.toInt(),
        0xFF2C3E50.toInt() to 0xFFFD746C.toInt(),
    )

    private fun save(bitmap: Bitmap, file: File): String {
        FileOutputStream(file).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        return file.absolutePath
    }

    /**
     * The sample a template ships with, copied into the project: "p1.jpg" and on for its places,
     * "music.m4a" for its sound. Null when it has none, and a place then gets [placeholder].
     */
    fun sample(context: android.content.Context, dir: File, id: String, name: String): String? {
        val asset = "templates/$id/$name"
        val there = runCatching { context.assets.list("templates/$id")?.contains(name) == true }.getOrDefault(false)
        if (!there) return null
        val file = File(dir, "template-$id-$name")
        context.assets.open(asset).use { input -> FileOutputStream(file).use { input.copyTo(it) } }
        return file.absolutePath
    }

    /** The picture standing in for place [n] (from 1) until a photo of the user's goes there. */
    fun placeholder(dir: File, n: Int): String {
        val b = Bitmap.createBitmap(1080, 1920, Bitmap.Config.ARGB_8888)
        val c = Canvas(b)
        val (top, bottom) = HUES[(n - 1) % HUES.size]
        c.drawRect(0f, 0f, 1080f, 1920f, Paint().apply { shader = LinearGradient(0f, 0f, 1080f, 1920f, top, bottom, Shader.TileMode.CLAMP) })
        val ink = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xE6FFFFFF.toInt(); textAlign = Paint.Align.CENTER; typeface = Typeface.create(Typeface.SANS_SERIF, Typeface.BOLD) }
        // a person's outline, so a template that works with people shows what it does
        val shade = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0x33FFFFFF }
        c.drawCircle(540f, 760f, 150f, shade)
        c.drawRoundRect(RectF(330f, 940f, 750f, 1600f), 200f, 200f, shade)
        ink.textSize = 220f
        c.drawText("$n", 540f, 840f, ink)
        ink.textSize = 64f
        c.drawText("Your photo or video", 540f, 1720f, ink)
        return save(b, File(dir, "template-place-$n.png"))
    }

    /** One of a template's graphics, white on clear, by its name: "ring", "frame", "heart", or a "panel-…" half of the frame. */
    fun graphic(dir: File, name: String): String {
        if (name.startsWith("panel-")) return save(panel(name.removePrefix("panel-")), File(dir, "sticker-template-$name.png"))
        val b = Bitmap.createBitmap(1024, 1024, Bitmap.Config.ARGB_8888)
        val c = Canvas(b)
        val line = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFFFFFFFF.toInt(); style = Paint.Style.STROKE; strokeWidth = 26f; strokeJoin = Paint.Join.ROUND; strokeCap = Paint.Cap.ROUND }
        when (name) {
            "ring" -> c.drawCircle(512f, 512f, 440f, line)
            "frame" -> c.drawRoundRect(RectF(150f, 40f, 874f, 984f), 36f, 36f, line)
            else -> c.drawPath(Path().apply {
                moveTo(512f, 930f)
                cubicTo(80f, 650f, 60f, 230f, 320f, 170f)
                cubicTo(420f, 150f, 490f, 210f, 512f, 290f)
                cubicTo(534f, 210f, 604f, 150f, 704f, 170f)
                cubicTo(964f, 230f, 944f, 650f, 512f, 930f)
                close()
            }, line)
        }
        // the name tells the timeline it is a sticker, over the video
        return save(b, File(dir, "sticker-template-$name.png"))
    }

    /** The whole frame, clear but for a soft white over its [half]: "right", "left", "top" or "bottom". */
    private fun panel(half: String): Bitmap {
        val b = Bitmap.createBitmap(1080, 1920, Bitmap.Config.ARGB_8888)
        val area = when (half) {
            "right" -> RectF(540f, 0f, 1080f, 1920f)
            "left" -> RectF(0f, 0f, 540f, 1920f)
            "top" -> RectF(0f, 0f, 1080f, 960f)
            else -> RectF(0f, 960f, 1080f, 1920f)
        }
        Canvas(b).drawRect(area, Paint().apply { color = 0xC8FFFFFF.toInt() })
        return b
    }
}
