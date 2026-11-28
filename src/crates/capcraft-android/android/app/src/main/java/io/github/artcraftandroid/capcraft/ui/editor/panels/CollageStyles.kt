package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Shader
import java.io.File
import kotlin.math.ceil
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.random.Random

/**
 * Collage frames of Forge's own: each a way of placing the pictures (free boxes, turned or cut to a
 * shape) and a background drawn here in code - a film strip, prints on paper, hearts, confetti - for
 * any number of pictures. Nothing here is taken from another app's artwork.
 */
internal class CollageStyle(val id: String, val name: String)

internal val COLLAGE_STYLES = listOf(
    CollageStyle("", "Grid"),
    CollageStyle("film", "Film"),
    CollageStyle("prints", "Prints"),
    CollageStyle("love", "Love"),
    CollageStyle("shapes", "Shapes"),
    CollageStyle("party", "Party"),
    CollageStyle("floral", "Floral"),
    CollageStyle("classic", "Classic"),
)

/** A box in pixels, its turn in degrees and the shape its picture is cut to. */
private class Box(val x: Float, val y: Float, val w: Float, val h: Float, val rot: Float = 0f, val shape: String = "")

/** Rows and columns enough for [n], about as many across as down on a frame of [aspect]. */
private fun grid(n: Int, aspect: Float): Pair<Int, Int> {
    val cols = ceil(sqrt(n * aspect)).toInt().coerceIn(1, n)
    val rows = ceil(n / cols.toFloat()).toInt()
    return cols to rows
}

/** [n] boxes in rows inside `(x0, y0, w, h)`, each [fill] of its slot, the last row centred. */
private fun slots(n: Int, x0: Float, y0: Float, w: Float, h: Float, fill: Float, square: Boolean): List<Box> {
    val (cols, rows) = grid(n, w / h)
    val sw = w / cols
    val sh = h / rows
    return (0 until n).map { i ->
        val r = i / cols
        val inRow = if (r == rows - 1) n - r * cols else cols
        val c = i % cols
        val offset = (cols - inRow) * sw / 2
        var bw = sw * fill
        var bh = sh * fill
        if (square) { val s = minOf(bw, bh); bw = s; bh = s }
        Box(x0 + offset + c * sw + (sw - bw) / 2, y0 + r * sh + (sh - bh) / 2, bw, bh)
    }
}

/** Where [style]'s [n] pictures go on a [w] x [h] frame. */
private fun boxes(style: String, n: Int, w: Float, h: Float): List<Box> {
    val m = minOf(w, h)
    return when (style) {
        // a strip down the middle (two for more than four), a frame per picture between the sprockets
        "film" -> {
            val strips = if (n > 4) 2 else 1
            val per = ceil(n / strips.toFloat()).toInt()
            val stripW = minOf(w * 0.8f / strips, h / per * 1.35f)
            val gap = (w - stripW * strips) / (strips + 1)
            (0 until n).map { i ->
                val s = i / per
                val k = i % per
                val sx = gap + s * (stripW + gap)
                val frameH = (h - m * 0.08f) / per
                Box(sx + stripW * 0.16f, m * 0.04f + k * frameH + frameH * 0.06f, stripW * 0.68f, frameH * 0.88f)
            }
        }
        // prints, a little turned each way
        "prints" -> slots(n, m * 0.08f, m * 0.08f, w - m * 0.16f, h - m * 0.16f, 0.78f, false).mapIndexed { i, b ->
            Box(b.x, b.y, b.w, b.h * 0.86f, if (i % 2 == 0) -5f + i else 4f - i * 0.5f)
        }
        "love" -> slots(n, m * 0.06f, m * 0.06f, w - m * 0.12f, h - m * 0.12f, 0.92f, true).mapIndexed { i, b ->
            Box(b.x, b.y, b.w, b.h, if (n > 1) (if (i % 2 == 0) -6f else 6f) else 0f, "heart")
        }
        "shapes" -> slots(n, m * 0.05f, m * 0.05f, w - m * 0.1f, h - m * 0.1f, 0.9f, true).mapIndexed { i, b ->
            Box(b.x, b.y, b.w, b.h, 0f, listOf("circle", "star", "round", "circle", "heart")[i % 5])
        }
        "party", "floral" -> slots(n, m * 0.1f, m * 0.1f, w - m * 0.2f, h - m * 0.2f, 0.86f, false).map {
            Box(it.x, it.y, it.w, it.h, 0f, "round")
        }
        // one big picture with wide white margins, the rest small beneath it
        "classic" -> {
            val pad = m * 0.08f
            if (n == 1) listOf(Box(pad, pad, w - 2 * pad, h - 2 * pad))
            else {
                val bigH = (h - 3 * pad) * 0.62f
                listOf(Box(pad, pad, w - 2 * pad, bigH)) +
                    slots(n - 1, pad, 2 * pad + bigH, w - 2 * pad, h - 3 * pad - bigH, 0.9f, false)
            }
        }
        else -> emptyList()
    }
}

/** [style]'s cells for [n] pictures on a [w] x [h] frame, in fractions of the frame. */
internal fun styleCells(style: String, n: Int, w: Int, h: Int): List<Cell> =
    boxes(style, n, w.toFloat(), h.toFloat()).map { Cell(it.x / w, it.y / h, it.w / w, it.h / h, it.rot, it.shape) }

private fun paint(color: Int, style: Paint.Style = Paint.Style.FILL, width: Float = 0f) =
    Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = color; this.style = style; strokeWidth = width }

/** A heart of width [s] centred on `(cx, cy)`. */
private fun heart(cx: Float, cy: Float, s: Float): Path = Path().apply {
    for (k in 0..60) {
        val t = k / 60.0 * 2 * Math.PI
        val x = 16 * sin(t) * sin(t) * sin(t)
        val y = 13 * cos(t) - 5 * cos(2 * t) - 2 * cos(3 * t) - cos(4 * t)
        val px = cx + (x / 34 * s).toFloat()
        val py = cy - (y / 34 * s).toFloat()
        if (k == 0) moveTo(px, py) else lineTo(px, py)
    }
    close()
}

/** Draws [b]'s rectangle (or rounded one) on [c], turned as it is. */
private fun Canvas.boxed(b: Box, grow: Float, radius: Float, p: Paint) {
    save()
    rotate(b.rot, b.x + b.w / 2, b.y + b.h / 2)
    drawRoundRect(RectF(b.x - grow, b.y - grow, b.x + b.w + grow, b.y + b.h + grow), radius, radius, p)
    restore()
}

/** How many frame backgrounds a project keeps: each is a whole frame, so they are not kept for good. */
private const val KEPT_BACKGROUNDS = 24

/**
 * Draws [style]'s background for [n] pictures on a [w] x [h] frame into a PNG in [dir], and gives its
 * path. The same drawing for the same style, count and size is kept and reused.
 */
internal fun styleBackground(style: String, n: Int, w: Int, h: Int, media: File): String {
    // a folder of their own: the media folder holds the user's imports, under their own names
    val dir = File(media, "collage-frames").apply { mkdirs() }
    val file = File(dir, "$style-$n-${w}x$h.png")
    // in use again: newest, so the clean-up below keeps it
    if (file.exists()) return file.absolutePath.also { file.setLastModified(System.currentTimeMillis()) }
    val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val c = Canvas(bmp)
    val fw = w.toFloat()
    val fh = h.toFloat()
    val m = minOf(fw, fh)
    val rnd = Random(n * 31 + style.hashCode())
    val bs = boxes(style, n, fw, fh)
    when (style) {
        "film" -> {
            c.drawColor(0xFF1C1C1E.toInt())
            val strips = if (n > 4) 2 else 1
            val per = ceil(n / strips.toFloat()).toInt()
            val stripW = minOf(fw * 0.8f / strips, fh / per * 1.35f)
            val gap = (fw - stripW * strips) / (strips + 1)
            val hole = paint(0xFFE9E4DA.toInt())
            for (s in 0 until strips) {
                val sx = gap + s * (stripW + gap)
                c.drawRect(sx, 0f, sx + stripW, fh, paint(0xFF050505.toInt()))
                // sprocket holes down both edges
                val hw = stripW * 0.07f
                val hh = hw * 1.3f
                var y = hh
                while (y < fh - hh) {
                    for (hx in listOf(sx + stripW * 0.045f, sx + stripW * 0.955f - hw)) c.drawRoundRect(RectF(hx, y, hx + hw, y + hh), hw * 0.25f, hw * 0.25f, hole)
                    y += hh * 2.1f
                }
            }
        }
        "prints" -> {
            c.drawColor(0xFFEFE6D8.toInt())
            val shadow = paint(0x33000000)
            val white = paint(0xFFFFFFFF.toInt())
            bs.forEach { b ->
                val grow = m * 0.018f
                c.boxed(Box(b.x + m * 0.008f, b.y + m * 0.012f, b.w, b.h, b.rot), grow, 2f, shadow)
                // a print's border, wider at the foot
                c.save()
                c.rotate(b.rot, b.x + b.w / 2, b.y + b.h / 2)
                c.drawRect(b.x - grow, b.y - grow, b.x + b.w + grow, b.y + b.h + grow * 4.5f, white)
                c.restore()
            }
        }
        "love" -> {
            val g = paint(0xFF000000.toInt())
            g.shader = LinearGradient(0f, 0f, 0f, fh, 0xFFFFD6E0.toInt(), 0xFFFF8FAB.toInt(), Shader.TileMode.CLAMP)
            c.drawRect(0f, 0f, fw, fh, g)
            val doodle = paint(0xAAFFFFFF.toInt(), Paint.Style.STROKE, m * 0.006f)
            repeat(26) { c.drawPath(heart(rnd.nextFloat() * fw, rnd.nextFloat() * fh, m * (0.03f + rnd.nextFloat() * 0.05f)), doodle) }
        }
        "shapes" -> {
            val g = paint(0xFF000000.toInt())
            g.shader = LinearGradient(0f, 0f, fw, fh, 0xFF8EC5FC.toInt(), 0xFFE0C3FC.toInt(), Shader.TileMode.CLAMP)
            c.drawRect(0f, 0f, fw, fh, g)
            val blob = paint(0x33FFFFFF)
            repeat(9) { c.drawCircle(rnd.nextFloat() * fw, rnd.nextFloat() * fh, m * (0.05f + rnd.nextFloat() * 0.12f), blob) }
        }
        "party" -> {
            c.drawColor(0xFFFFF4D6.toInt())
            // gold stripes across the corner, and confetti
            val stripe = paint(0x44E0A526)
            var x = -fh
            while (x < fw) {
                c.drawPath(Path().apply { moveTo(x, fh); lineTo(x + m * 0.05f, fh); lineTo(x + fh + m * 0.05f, 0f); lineTo(x + fh, 0f); close() }, stripe)
                x += m * 0.12f
            }
            val colours = listOf(0xFFFF5D8F, 0xFF4CC9F0, 0xFFFFC300, 0xFF7B2FF7, 0xFF2EC4B6).map { it.toInt() }
            repeat(90) { c.drawCircle(rnd.nextFloat() * fw, rnd.nextFloat() * fh, m * (0.004f + rnd.nextFloat() * 0.008f), paint(colours[it % colours.size])) }
            val gold = paint(0xFFD4A017.toInt(), Paint.Style.STROKE, m * 0.012f)
            bs.forEach { b -> c.boxed(b, m * 0.012f, minOf(b.w, b.h) * 0.14f, gold) }
        }
        "floral" -> {
            c.drawColor(0xFFEAF6EF.toInt())
            val leaf = paint(0x5578B48C)
            val dot = paint(0x88F4A6A6.toInt())
            repeat(30) {
                val cx = rnd.nextFloat() * fw
                val cy = rnd.nextFloat() * fh
                c.save()
                c.rotate(rnd.nextFloat() * 180f, cx, cy)
                c.drawOval(RectF(cx - m * 0.035f, cy - m * 0.012f, cx + m * 0.035f, cy + m * 0.012f), leaf)
                c.restore()
                c.drawCircle(rnd.nextFloat() * fw, rnd.nextFloat() * fh, m * 0.008f, dot)
            }
            val edge = paint(0xFFF08A7E.toInt(), Paint.Style.STROKE, m * 0.008f)
            bs.forEach { b -> c.boxed(b, m * 0.012f, minOf(b.w, b.h) * 0.14f, edge) }
        }
        "classic" -> {
            c.drawColor(0xFFFFFFFF.toInt())
            val line = paint(0xFFDDDDDD.toInt(), Paint.Style.STROKE, m * 0.002f)
            bs.forEach { b -> c.boxed(b, m * 0.006f, 0f, line) }
        }
        else -> c.drawColor(0xFFFFFFFF.toInt())
    }
    file.outputStream().use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
    bmp.recycle()
    // the newest few kept, enough for undo to find the ones it goes back to; the rest go
    dir.listFiles { f -> f.name.endsWith(".png") }
        ?.sortedByDescending { it.lastModified() }
        ?.drop(KEPT_BACKGROUNDS)
        ?.forEach { it.delete() }
    return file.absolutePath
}
