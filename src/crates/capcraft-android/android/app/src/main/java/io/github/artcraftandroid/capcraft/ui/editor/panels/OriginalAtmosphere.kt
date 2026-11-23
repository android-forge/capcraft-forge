package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RadialGradient
import android.graphics.Shader
import android.net.Uri
import java.io.File
import java.util.Random
import kotlin.math.sin

/** Procedural, original atmosphere artwork. It is generated locally and never embeds the source bundle. */
internal object OriginalAtmosphere {
    private enum class Form { CLOUD, DUST, FULL, GROUND, LASER, RAYS, LIGHTS, MIST, NEBULA, SLOPE, SMOKE, STEAM, STRUCTURE, WATER, WINDOW }
    private data class Group(val label: String, val form: Form, val palette: IntArray, val looks: List<String>)

    private val groups = listOf(
        Group("Clouds", Form.CLOUD, intArrayOf(0xFFD5E5F2.toInt(), 0xFFC5D8E8.toInt(), 0xFFFFD9B2.toInt()), listOf("Soft billows", "Golden horizon", "Storm shelf", "Cloud bank", "High haze", "Sunlit edge")),
        Group("Dust", Form.DUST, intArrayOf(0xFFFFD58A.toInt(), 0xFFE8D2AE.toInt(), 0xFFC6D9F2.toInt()), listOf("Fine motes", "Backlit dust", "Floating specks", "Dense drift", "Golden particles", "Wide sparkle")),
        Group("Fullscreen", Form.FULL, intArrayOf(0xFFB9C9D8.toInt(), 0xFFD8C5B9.toInt(), 0xFFAFC9C4.toInt()), listOf("Soft veil", "Smoky wash", "Airy haze", "Rolling clouds", "Cool atmosphere", "Warm atmosphere")),
        Group("Ground smoke", Form.GROUND, intArrayOf(0xFFDCE0E7.toInt(), 0xFFC6D6E2.toInt(), 0xFFE7D4C6.toInt()), listOf("Low roll", "Thin ground fog", "Dense bank", "Stage fog", "Drifting layer", "Heavy plume")),
        Group("Lasers", Form.LASER, intArrayOf(0xFFFF386D.toInt(), 0xFF45E9FF.toInt(), 0xFFB96BFF.toInt()), listOf("Neon sweep", "Crossing beams", "Diagonal streaks", "Red scan", "Cyan scan", "Violet burst")),
        Group("Light rays", Form.RAYS, intArrayOf(0xFFFFE7B0.toInt(), 0xFFFFD5A0.toInt(), 0xFFD6E7FF.toInt()), listOf("God rays", "Soft shafts", "Sun fan", "Side light", "Wide beams", "Bright opening")),
        Group("Lights", Form.LIGHTS, intArrayOf(0xFFFFB267.toInt(), 0xFFFF7E9F.toInt(), 0xFF7EDBFF.toInt()), listOf("Warm bokeh", "Neon bokeh", "Cool bokeh", "Soft circles", "Lens glow", "Color lights")),
        Group("Mist", Form.MIST, intArrayOf(0xFFD4E3E8.toInt(), 0xFFC8D7E8.toInt(), 0xFFE8D8C5.toInt()), listOf("Valley mist", "Low fog", "Thin haze", "Rolling fog", "Cool morning", "Warm morning")),
        Group("Nebula", Form.NEBULA, intArrayOf(0xFFB957FF.toInt(), 0xFF438CFF.toInt(), 0xFFFF5C9C.toInt()), listOf("Violet cloud", "Blue cloud", "Pink cloud", "Cosmic swirl", "Deep space", "Color burst")),
        Group("Slope smoke", Form.SLOPE, intArrayOf(0xFFD6DDE3.toInt(), 0xFFBFD2DF.toInt(), 0xFFE6CFC4.toInt()), listOf("Diagonal drift", "Rising sweep", "Side plume", "Low sweep", "Tall slope", "Dense diagonal")),
        Group("Smoke", Form.SMOKE, intArrayOf(0xFFD5D8DE.toInt(), 0xFFC3CED8.toInt(), 0xFFE3D4C9.toInt()), listOf("Wispy smoke", "Billowing smoke", "Soft plume", "Wide plume", "Dense curl", "Layered smoke")),
        Group("Steam", Form.STEAM, intArrayOf(0xFFE4E7EC.toInt(), 0xFFD4E3E9.toInt(), 0xFFF0D9C6.toInt()), listOf("Rising steam", "Soft plume", "Thin curls", "Wide vapor", "Tall column", "Warm steam")),
        Group("Structure smoke", Form.STRUCTURE, intArrayOf(0xFFC9D2DB.toInt(), 0xFFD9CEC8.toInt(), 0xFFB8D4D5.toInt()), listOf("Twisting column", "Layered curl", "Broken plume", "Tall structure", "Wide structure", "Dense column")),
        Group("Underwater rays", Form.WATER, intArrayOf(0xFF55D9E8.toInt(), 0xFF65AFFF.toInt(), 0xFF74E9C2.toInt()), listOf("Blue caustics", "Shallow rays", "Deep rays", "Surface fan", "Green water", "Bright caustics")),
        Group("Window light beams", Form.WINDOW, intArrayOf(0xFFFFE6B2.toInt(), 0xFFFFDDB0.toInt(), 0xFFD5E8FF.toInt()), listOf("Warm window", "Soft window", "Cool window", "Wide window", "Side window", "Bright room")),
    )

    private const val WIDTH = 768
    private const val HEIGHT = 512

    /** Six locally generated choices for each of the fifteen atmosphere families. */
    fun files(context: Context): List<OverlayFile> {
        val dir = File(context.filesDir, "original-atmosphere").apply { mkdirs() }
        return buildList {
            groups.forEachIndexed { groupIndex, group ->
                group.looks.forEachIndexed { variation, look ->
                    val name = look
                    val file = File(dir, "%02d-%02d.jpg".format(groupIndex, variation))
                    if (!file.isFile || file.length() == 0L) runCatching { render(file, group, variation, 0x43465000L + groupIndex * 101L + variation * 7919L) }
                    if (file.isFile && file.length() > 0L) add(OverlayFile(Uri.fromFile(file), name, "Original · ${group.label}"))
                }
            }
        }
    }

    private fun render(file: File, group: Group, variation: Int, seed: Long) {
        val bitmap = Bitmap.createBitmap(WIDTH, HEIGHT, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(android.graphics.Color.BLACK)
        val random = Random(seed)
        val color = group.palette[random.nextInt(group.palette.size)]
        when (group.form) {
            Form.DUST -> dust(canvas, random, color, 470 + variation * 90)
            Form.LASER -> lasers(canvas, random, color, variation)
            Form.RAYS, Form.WATER, Form.WINDOW -> beams(canvas, random, color, group.form, variation)
            Form.LIGHTS -> bokeh(canvas, random, group.palette, 65 + variation * 9)
            Form.NEBULA -> clouds(canvas, random, group.palette, 62 + variation * 12, 1.5f + variation * 0.04f, 0.42f, 0.72f)
            Form.CLOUD -> clouds(canvas, random, group.palette, 72 + variation * 9, 1.15f + variation * 0.035f, 0.32f, 0.56f + (variation % 3) * 0.08f)
            Form.MIST -> vapor(canvas, random, color, 0.48f + (variation % 3) * 0.06f, 0.12f + (variation % 3) * 0.12f, 0.22f + variation * 0.025f, 0.55f + (variation % 3) * 0.08f, variation)
            Form.GROUND -> vapor(canvas, random, color, 0.78f + (variation % 3) * 0.035f, 0.12f + (variation % 3) * 0.1f, 0.2f + variation * 0.025f, 0.48f + (variation % 3) * 0.07f, variation)
            Form.STEAM -> vapor(canvas, random, color, 0.82f, -0.5f - (variation % 3) * 0.15f, 0.09f + variation * 0.02f, 0.4f + (variation % 3) * 0.07f, variation)
            Form.SLOPE -> vapor(canvas, random, color, 0.72f + (variation % 3) * 0.04f, -0.6f - (variation % 3) * 0.1f, 0.22f + variation * 0.02f, 0.48f + (variation % 3) * 0.08f, variation)
            Form.STRUCTURE -> vapor(canvas, random, color, 0.80f, -0.18f - (variation % 3) * 0.12f, 0.16f + variation * 0.02f, 0.72f + (variation % 3) * 0.08f, variation)
            Form.SMOKE -> vapor(canvas, random, color, 0.60f + (variation % 3) * 0.07f, 0.2f + (variation % 3) * 0.18f, 0.3f + variation * 0.025f, 0.62f + (variation % 3) * 0.07f, variation)
            Form.FULL -> {
                clouds(canvas, random, group.palette, 60 + variation * 10, 1.5f, 0.22f, 0.78f)
                vapor(canvas, random, color, 0.55f, 0.3f + (variation % 3) * 0.12f, 0.15f + variation * 0.02f, 0.78f, variation)
            }
        }
        file.outputStream().buffered().use { bitmap.compress(Bitmap.CompressFormat.JPEG, 91, it) }
        bitmap.recycle()
    }

    private fun paint(color: Int, alpha: Int = 255, style: Paint.Style = Paint.Style.FILL) = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        this.color = color
        this.alpha = alpha
        this.style = style
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
    }

    private fun glow(canvas: Canvas, x: Float, y: Float, rx: Float, ry: Float, color: Int, alpha: Int) {
        val r = maxOf(rx, ry).coerceAtLeast(1f)
        val p = paint(color).apply {
            shader = RadialGradient(x, y, r, intArrayOf((alpha shl 24) or (color and 0x00FFFFFF), 0x00000000), null, Shader.TileMode.CLAMP)
        }
        canvas.save()
        canvas.scale(rx / r, ry / r, x, y)
        canvas.drawCircle(x, y, r, p)
        canvas.restore()
    }

    private fun clouds(canvas: Canvas, random: Random, palette: IntArray, count: Int, spread: Float, yCenter: Float, yHeight: Float = 0.60f) {
        repeat(count) {
            val x = random.nextFloat() * WIDTH
            val y = ((yCenter + (random.nextFloat() - 0.5f) * yHeight) * HEIGHT).coerceIn(0f, HEIGHT.toFloat())
            val r = 22f + random.nextFloat() * 105f * spread
            glow(canvas, x, y, r * (0.65f + random.nextFloat() * 0.8f), r * (0.38f + random.nextFloat() * 0.65f), palette[random.nextInt(palette.size)], 10 + random.nextInt(28))
        }
    }

    private fun vapor(canvas: Canvas, random: Random, color: Int, start: Float, drift: Float, width: Float, height: Float, variation: Int) {
        val strands = 7 + variation / 3 * 2 + random.nextInt(5)
        repeat(strands) { strand ->
            val phase = random.nextFloat() * 6.28f
            val baseX = random.nextFloat() * WIDTH
            val steps = 18
            for (i in 0..steps) {
                val t = i.toFloat() / steps
                val sweep = sin(t * 5.4f + phase) * drift * WIDTH * 0.20f
                val x = baseX + sweep + (t - 0.5f) * width * WIDTH * (random.nextFloat() * 0.3f + 0.7f)
                val y = (start + (t - 0.5f) * height) * HEIGHT
                val r = (24f + random.nextFloat() * 65f) * (0.45f + sin(t * Math.PI).toFloat() * 0.65f)
                val alpha = (10 + random.nextInt(24) + (strand % 3) * 3).coerceAtMost(40)
                glow(canvas, x, y, r * 1.35f, r * 0.85f, color, alpha)
            }
        }
        // Fine curls give each puff bank a hand-shaped edge.
        repeat(9 + variation / 3 * 3) {
            val p = Path()
            val x = random.nextFloat() * WIDTH
            val y = (start + (random.nextFloat() - 0.5f) * height) * HEIGHT
            p.moveTo(x, y)
            p.cubicTo(x - 65f, y - 80f, x + random.nextInt(150) - 75f, y + 70f, x + random.nextInt(210) - 105f, y - 18f)
            canvas.drawPath(p, paint(color, 18 + random.nextInt(25), Paint.Style.STROKE).apply { strokeWidth = 3f + random.nextFloat() * 8f })
        }
    }

    private fun dust(canvas: Canvas, random: Random, color: Int, count: Int) {
        repeat(count) {
            val x = random.nextFloat() * WIDTH
            val y = random.nextFloat() * HEIGHT
            val r = 0.8f + random.nextFloat().pow2() * 6f
            val a = 45 + random.nextInt(165)
            if (r > 3.2f) glow(canvas, x, y, r * 2.8f, r * 2.8f, color, a / 3)
            canvas.drawCircle(x, y, r, paint(color, a))
        }
    }

    private fun lasers(canvas: Canvas, random: Random, color: Int, variation: Int) {
        repeat(7 + variation * 2) {
            val y = HEIGHT * (0.12f + random.nextFloat() * 0.76f)
            val x0 = -40f
            val x1 = WIDTH + 40f
            val bend = (random.nextFloat() - 0.5f) * 120f
            val tilt = when (variation % 3) { 1 -> HEIGHT * 0.38f; 2 -> -HEIGHT * 0.38f; else -> 0f }
            val p = Path().apply { moveTo(x0, y); cubicTo(WIDTH * 0.28f, y + bend + tilt * 0.3f, WIDTH * 0.71f, y - bend + tilt * 0.7f, x1, y + bend * 0.28f + tilt) }
            canvas.drawPath(p, paint(color, 38, Paint.Style.STROKE).apply { strokeWidth = 24f })
            canvas.drawPath(p, paint(color, 130, Paint.Style.STROKE).apply { strokeWidth = 4f })
            canvas.drawPath(p, paint(0xFFFFFFFF.toInt(), 215, Paint.Style.STROKE).apply { strokeWidth = 1.5f })
        }
    }

    private fun beams(canvas: Canvas, random: Random, color: Int, form: Form, variation: Int) {
        val source = floatArrayOf(0.2f, 0.5f, 0.8f)[variation % 3]
        val originX = WIDTH * (source + (random.nextFloat() - 0.5f) * 0.12f)
        val originY = if (form == Form.WINDOW) -30f else -40f
        val count = 7 + variation * 2
        repeat(count) { i ->
            val spread = (i - (count - 1) / 2f) * (27f + random.nextFloat() * 18f)
            val endY = HEIGHT * (0.64f + random.nextFloat() * 0.55f)
            val endX = originX + spread * (if (form == Form.WINDOW) 2.2f else 1.25f)
            val half = (18f + random.nextFloat() * 27f) * (if (form == Form.WINDOW) 2.0f else 1.0f)
            val path = Path().apply {
                moveTo(originX - half * 0.08f, originY)
                lineTo(endX - half, endY)
                lineTo(endX + half, endY)
                lineTo(originX + half * 0.08f, originY)
                close()
            }
            val beamColor = if (form == Form.WATER) 0xFF74DDF0.toInt() else color
            fun alpha(a: Int) = (a shl 24) or (beamColor and 0x00FFFFFF)
            val beamPaint = paint(beamColor).apply {
                shader = LinearGradient(originX, originY, endX, endY, intArrayOf(alpha(0x24), alpha(0x60), alpha(0)), floatArrayOf(0f, 0.22f, 1f), Shader.TileMode.CLAMP)
            }
            canvas.drawPath(path, beamPaint)
            if (form == Form.WATER) {
                val p = Path().apply {
                    moveTo(endX - half * 1.8f, endY - 10f)
                    for (x in 0..8) lineTo(endX - half * 1.8f + x * half * 0.45f, endY - 10f + sin(x * 1.7f + i) * 8f)
                }
                canvas.drawPath(p, paint(0xFF8CF4F3.toInt(), 95, Paint.Style.STROKE).apply { strokeWidth = 2.2f })
            }
        }
        glow(canvas, originX, 18f, 70f, 48f, color, 105)
    }

    private fun bokeh(canvas: Canvas, random: Random, palette: IntArray, count: Int) {
        repeat(count) {
            val x = random.nextFloat() * WIDTH
            val y = random.nextFloat() * HEIGHT
            val r = 6f + random.nextFloat() * 43f
            val c = palette[random.nextInt(palette.size)]
            glow(canvas, x, y, r * 1.8f, r * 1.8f, c, 35 + random.nextInt(75))
            canvas.drawCircle(x, y, r, paint(c, 40 + random.nextInt(95), Paint.Style.STROKE).apply { strokeWidth = 1f + random.nextFloat() * 2f })
        }
    }

    private fun Float.pow2() = this * this
}
