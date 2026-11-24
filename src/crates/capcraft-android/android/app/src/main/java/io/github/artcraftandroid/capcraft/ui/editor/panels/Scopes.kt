// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft Forge contributors

package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.ui.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import kotlin.math.max
import kotlin.math.roundToInt

/** The scope shown: a tap on it moves to the next. */
private enum class Scope(val label: String) { Histogram("Histogram"), Waveform("Waveform"), Vectorscope("Vectorscope") }

/** Columns the picture is read at: enough to see the shape of the light, cheap enough to read five times a second. */
private const val ACROSS = 128

/** Levels a scope sorts the light into. */
private const val LEVELS = 64

/** How often the scope reads the picture, milliseconds. */
private const val EVERY_MS = 200L

/** What one reading of the picture found, for one scope. */
private class Reading(val scope: Scope, val counts: Array<IntArray>, val most: Int)

/**
 * The video scopes over the preview's corner while grading: a histogram of red, green, blue and light, a waveform
 * of the light across the picture, or a vectorscope of its colours. Read from the picture the preview shows, made
 * small, off the main thread; a tap moves to the next scope.
 */
@Composable
fun ScopesOverlay(image: ImageBitmap?, modifier: Modifier = Modifier) {
    var scope by remember { mutableStateOf(Scope.Histogram) }
    var reading by remember { mutableStateOf<Reading?>(null) }
    val picture by rememberUpdatedState(image)
    val chosen by rememberUpdatedState(scope)
    LaunchedEffect(Unit) {
        while (isActive) {
            val shown = picture
            if (shown != null) {
                reading = withContext(Dispatchers.Default) { runCatching { read(shown.asAndroidBitmap(), chosen) }.getOrNull() } ?: reading
            }
            delay(EVERY_MS)
        }
    }
    Box(
        modifier.padding(8.dp).size(176.dp, 112.dp).clip(RoundedCornerShape(8.dp)).background(Color(0xD0101010))
            .clickable { scope = Scope.entries[(scope.ordinal + 1) % Scope.entries.size] },
    ) {
        val now = reading?.takeIf { it.scope == scope }
        Canvas(Modifier.fillMaxSize().padding(start = 6.dp, end = 6.dp, top = 16.dp, bottom = 6.dp)) {
            when (scope) {
                Scope.Histogram -> {
                    val colours = listOf(Color(0xFFFF5252), Color(0xFF69F0AE), Color(0xFF448AFF), Color(0xFFEEEEEE))
                    now?.counts?.forEachIndexed { channel, bins ->
                        val path = Path()
                        bins.forEachIndexed { i, n ->
                            val x = size.width * i / (LEVELS - 1)
                            val y = size.height * (1f - n.toFloat() / max(1, now.most))
                            if (i == 0) path.moveTo(x, y) else path.lineTo(x, y)
                        }
                        drawPath(path, colours[channel].copy(alpha = if (channel == 3) 0.9f else 0.75f), style = Stroke(width = 1.5f))
                    }
                }
                Scope.Waveform -> {
                    // light from black at the bottom to white at the top, column by column of the picture
                    for (level in listOf(0.25f, 0.5f, 0.75f)) {
                        drawLine(Color(0x33FFFFFF), Offset(0f, size.height * (1 - level)), Offset(size.width, size.height * level.let { 1 - it }))
                    }
                    val cellW = size.width / ACROSS
                    val cellH = size.height / LEVELS
                    now?.counts?.forEachIndexed { column, levels ->
                        levels.forEachIndexed { level, n ->
                            if (n > 0) {
                                val alpha = (0.15f + 0.85f * n / max(1, now.most)).coerceAtMost(1f)
                                drawRect(Color(0xFF9CFF9C).copy(alpha = alpha), Offset(column * cellW, size.height - (level + 1) * cellH), Size(max(1f, cellW), max(1f, cellH)))
                            }
                        }
                    }
                }
                Scope.Vectorscope -> {
                    val side = minOf(size.width, size.height)
                    val left = (size.width - side) / 2
                    val centre = Offset(left + side / 2, side / 2)
                    drawCircle(Color(0x44FFFFFF), side / 2, centre, style = Stroke(width = 1f))
                    drawLine(Color(0x33FFFFFF), Offset(left, side / 2), Offset(left + side, side / 2))
                    drawLine(Color(0x33FFFFFF), Offset(left + side / 2, 0f), Offset(left + side / 2, side))
                    // the skin-tone line: where faces of every kind fall
                    drawLine(Color(0x66FFCC80), centre, Offset(centre.x - side * 0.26f, centre.y - side * 0.38f))
                    val cell = side / LEVELS
                    now?.counts?.forEachIndexed { u, column ->
                        column.forEachIndexed { v, n ->
                            if (n > 0) {
                                val alpha = (0.2f + 0.8f * n / max(1, now.most)).coerceAtMost(1f)
                                drawRect(Color.White.copy(alpha = alpha), Offset(left + u * cell, (LEVELS - 1 - v) * cell), Size(max(1f, cell), max(1f, cell)))
                            }
                        }
                    }
                }
            }
        }
        Text(scope.label, style = Type.Small.copy(fontSize = 10.sp, color = Color(0xCCFFFFFF)), modifier = Modifier.align(Alignment.TopStart).padding(start = 7.dp, top = 2.dp))
    }
}

/** One reading of [bitmap] for [scope]. */
private fun read(bitmap: Bitmap, scope: Scope): Reading? {
    if (bitmap.isRecycled || bitmap.width < 2 || bitmap.height < 2) return null
    val tall = (ACROSS * bitmap.height.toFloat() / bitmap.width).roundToInt().coerceIn(8, 256)
    val small = Bitmap.createScaledBitmap(bitmap, ACROSS, tall, true)
    val pixels = IntArray(ACROSS * tall)
    small.getPixels(pixels, 0, ACROSS, 0, 0, ACROSS, tall)
    if (small !== bitmap) small.recycle()
    fun bin(value: Float) = (value * (LEVELS - 1)).roundToInt().coerceIn(0, LEVELS - 1)
    return when (scope) {
        Scope.Histogram -> {
            val counts = Array(4) { IntArray(LEVELS) }
            for (p in pixels) {
                val r = (p shr 16 and 255) / 255f
                val g = (p shr 8 and 255) / 255f
                val b = (p and 255) / 255f
                counts[0][bin(r)]++
                counts[1][bin(g)]++
                counts[2][bin(b)]++
                counts[3][bin(0.2126f * r + 0.7152f * g + 0.0722f * b)]++
            }
            // the ends hold every clipped pixel: the shape between them sets the scale
            val most = counts.maxOf { bins -> (1 until LEVELS - 1).maxOf { bins[it] } }
            Reading(scope, counts, most)
        }
        Scope.Waveform -> {
            val counts = Array(ACROSS) { IntArray(LEVELS) }
            for (y in 0 until tall) for (x in 0 until ACROSS) {
                val p = pixels[y * ACROSS + x]
                val light = 0.2126f * (p shr 16 and 255) + 0.7152f * (p shr 8 and 255) + 0.0722f * (p and 255)
                counts[x][bin(light / 255f)]++
            }
            Reading(scope, counts, counts.maxOf { it.max() })
        }
        Scope.Vectorscope -> {
            val counts = Array(LEVELS) { IntArray(LEVELS) }
            for (p in pixels) {
                val r = (p shr 16 and 255) / 255f
                val g = (p shr 8 and 255) / 255f
                val b = (p and 255) / 255f
                // BT.709 colour difference, -0.5 to 0.5
                val cb = -0.1146f * r - 0.3854f * g + 0.5f * b
                val cr = 0.5f * r - 0.4542f * g - 0.0458f * b
                counts[bin(cb + 0.5f)][bin(cr + 0.5f)]++
            }
            Reading(scope, counts, counts.maxOf { it.max() })
        }
    }
}
