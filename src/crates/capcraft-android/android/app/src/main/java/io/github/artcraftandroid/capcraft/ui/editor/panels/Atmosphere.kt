package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.provider.DocumentsContract
import android.util.LruCache
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.core.ClipView
import io.github.artcraftandroid.capcraft.core.Laid
import io.github.artcraftandroid.capcraft.core.Motion
import io.github.artcraftandroid.capcraft.core.SpeechListener
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.engine.Media
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.intoBitmap
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Glyph
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import java.io.File
import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/** An overlay picture of the user's own, in a folder they chose. */
internal class OverlayFile(val uri: Uri, val name: String, val category: String)

/** Where an overlay goes on the frame. */
internal enum class Place { COVER, TOP, BOTTOM, MIDDLE }

/**
 * How one kind of overlay is laid: where, how strong on a picture of middling brightness, how much of the
 * frame its lit part is at the most, across and down, where it does not cover it, and whether it is light,
 * which takes the picture's warmth. Light of the sun's kind is [golden]: its beams yellow, and the picture warmed by them.
 */
internal class Preset(val words: List<String>, val place: Place, val strength: Float, val wide: Float = 1f, val tall: Float = 1f, val light: Boolean = false, val warmth: Float = 0f, val golden: Boolean = false)

private val PRESETS = listOf(
    Preset(listOf("ground smoke"), Place.BOTTOM, 0.85f, 1.1f, 0.5f),
    Preset(listOf("mist", "fog", "haze"), Place.BOTTOM, 0.8f, 1.35f, 0.5f),
    Preset(listOf("cloud"), Place.TOP, 0.9f, 0.95f, 0.55f),
    Preset(listOf("underwater"), Place.COVER, 0.85f, light = true, warmth = -0.5f),
    Preset(listOf("window"), Place.COVER, 0.8f, light = true, warmth = 0.5f, golden = true),
    Preset(listOf("ray", "beam", "flare", "sun"), Place.COVER, 0.85f, light = true, warmth = 0.4f, golden = true),
    Preset(listOf("laser"), Place.COVER, 0.95f, light = true),
    Preset(listOf("light"), Place.COVER, 0.9f, light = true, warmth = 0.2f, golden = true),
    Preset(listOf("dust", "particle", "bokeh", "snow"), Place.COVER, 0.9f),
    Preset(listOf("fullscreen", "full"), Place.COVER, 0.75f),
    Preset(listOf("nebula"), Place.COVER, 0.8f),
    Preset(listOf("steam"), Place.MIDDLE, 0.8f, 0.7f, 0.75f),
    Preset(listOf("slope", "smoke", "structure"), Place.MIDDLE, 0.8f, 0.75f, 0.8f),
)

/** An overlay of no known kind: over the whole frame, gently. */
private val ANY = Preset(emptyList(), Place.COVER, 0.7f)

internal fun presetFor(category: String): Preset =
    PRESETS.firstOrNull { p -> p.words.any { category.contains(it, ignoreCase = true) } } ?: ANY

/** What an overlay is, looked at small: the part of it that is lit (fractions), where its light is strongest, across and down, and how bright that is. */
internal class Seen(val left: Float, val top: Float, val right: Float, val bottom: Float, val heart: Float, val peak: Float, val aspect: Float, val heartY: Float = 0.5f)

internal fun seen(small: Bitmap): Seen {
    val w = small.width
    val h = small.height
    val px = IntArray(w * h).also { small.getPixels(it, 0, w, 0, 0, w, h) }
    val light = FloatArray(px.size) { i -> (0.299f * (px[i] shr 16 and 255) + 0.587f * (px[i] shr 8 and 255) + 0.114f * (px[i] and 255)) / 255f }
    var x0 = w
    var y0 = h
    var x1 = 0
    var y1 = 0
    var sum = 0f
    var across = 0f
    var down = 0f
    for (y in 0 until h) for (x in 0 until w) {
        val l = light[y * w + x]
        if (l > 0.07f) {
            x0 = min(x0, x)
            y0 = min(y0, y)
            x1 = max(x1, x + 1)
            y1 = max(y1, y + 1)
        }
        // the brightest of it is where the light comes from
        val k = l * l * l
        sum += k
        across += k * (x + 0.5f) / w
        down += k * (y + 0.5f) / h
    }
    if (x1 <= x0 || y1 <= y0) return Seen(0f, 0f, 1f, 1f, 0.5f, 0.1f, w.toFloat() / h)
    val sorted = light.sortedArray()
    return Seen(x0.toFloat() / w, y0.toFloat() / h, x1.toFloat() / w, y1.toFloat() / h, across / sum.coerceAtLeast(1e-6f), sorted[(sorted.size * 0.997f).toInt().coerceAtMost(sorted.size - 1)], w.toFloat() / h, down / sum.coerceAtLeast(1e-6f))
}

/** The picture under an overlay, looked at small: how bright it is, which side its light is on (-1 left to 1 right), and how warm it is (-1 to 1). */
internal class Scene(val bright: Float, val side: Float, val warm: Float)

internal fun sceneOf(small: Bitmap): Scene {
    val w = small.width
    val h = small.height
    val px = IntArray(w * h).also { small.getPixels(it, 0, w, 0, 0, w, h) }
    var sum = 0f
    var left = 0f
    var right = 0f
    var r = 0f
    var b = 0f
    for (i in px.indices) {
        val red = (px[i] shr 16 and 255) / 255f
        val green = (px[i] shr 8 and 255) / 255f
        val blue = (px[i] and 255) / 255f
        val l = 0.299f * red + 0.587f * green + 0.114f * blue
        sum += l
        if (i % w < w / 2) left += l * l else right += l * l
        r += red * l
        b += blue * l
    }
    val n = px.size.coerceAtLeast(1)
    return Scene(sum / n, ((right - left) / (right + left).coerceAtLeast(1e-4f)).coerceIn(-1f, 1f), ((r - b) / (r + b).coerceAtLeast(1e-4f) * 2f).coerceIn(-1f, 1f))
}

/**
 * How [seen], an overlay of kind [preset], is laid over [scene] on a frame [frameAspect] wide for its height:
 * its light coming from the side the picture's does, as strong as the picture's brightness bears, and lifted
 * where it is dim of itself.
 */
internal fun laid(preset: Preset, seen: Seen, scene: Scene, frameAspect: Float, golden: Boolean = false): Laid {
    // screened over a bright picture an overlay washes it out; over a dark one it hardly shows
    val bears = when {
        scene.bright < 0.25f -> 1.12f
        scene.bright < 0.5f -> 1.12f - (scene.bright - 0.25f) * 0.6f
        else -> (0.97f - (scene.bright - 0.5f) * 0.9f).coerceAtLeast(0.6f)
    }
    val opacity = (preset.strength * bears).coerceIn(0.25f, 1f)
    val flip = preset.light && abs(scene.side) > 0.06f && abs(seen.heart - 0.5f) > 0.08f && (seen.heart > 0.5f) != (scene.side > 0)
    val x0 = if (flip) 1f - seen.right else seen.left
    val x1 = if (flip) 1f - seen.left else seen.right
    // a scale of 1 fits the whole picture in the frame
    val fitW = if (seen.aspect >= frameAspect) 1f else seen.aspect / frameAspect
    val fitH = if (seen.aspect >= frameAspect) frameAspect / seen.aspect else 1f
    val cover = max(1f / fitW, 1f / fitH)
    val scale: Float
    var x = 0f
    var y = 0f
    if (preset.place == Place.COVER) {
        scale = cover * 1.02f
        if (golden && abs(scene.side) > 0.06f) {
            // where the picture is wider than the frame, the light comes in from the side the photo's does
            val across = fitW * scale
            val heart = if (flip) 1f - seen.heart else seen.heart
            val want = if (scene.side > 0) 0.84f else 0.16f
            val room = ((across - 1f) / 2f).coerceAtLeast(0f)
            x = (want - 0.5f - (heart - 0.5f) * across).coerceIn(-room, room)
        }
    } else {
        scale = min(preset.wide / ((x1 - x0) * fitW).coerceAtLeast(0.05f), preset.tall / ((seen.bottom - seen.top) * fitH).coerceAtLeast(0.05f))
            .coerceIn(0.4f, cover * 2.2f)
        val w = fitW * scale
        val h = fitH * scale
        // the middle of its lit part, from the middle of the frame
        x = -((x0 + x1) / 2f - 0.5f) * w
        val litTop = (seen.top - 0.5f) * h
        val litBottom = (seen.bottom - 0.5f) * h
        y = when (preset.place) {
            Place.TOP -> -0.5f + 0.06f - litTop
            Place.BOTTOM -> 0.5f + 0.03f - litBottom
            else -> -(litTop + litBottom) / 2f + 0.08f
        }
    }
    val adjust = JSONObject()
    if (!golden && seen.peak < 0.7f) adjust.put("exposure", (ln(0.8f / seen.peak.coerceAtLeast(0.05f)) / ln(2f)).coerceAtMost(if (preset.light) 2f else 1.3f).toDouble())
    // its light the colour of the picture's own, and of what it is: fewer kelvin is warmer
    val warm = (scene.warm * 0.6f + preset.warmth * 0.6f).coerceIn(-1f, 1f)
    if (!golden && preset.light && abs(warm) > 0.08f) adjust.put("temperature", (6500 - warm * 2200).roundToInt())
    return Laid("screen", opacity.toDouble(), scale.toDouble(), x.toDouble(), y.toDouble(), 0.0, flip, if (adjust.length() == 0) "" else adjust.toString())
}

/** The colour of golden light [l] bright (0 to 1), each channel 0 to 1: amber where it is faint, yellow in the beam, near white at its heart. */
internal fun goldenAt(l: Float): FloatArray {
    val t = (l / 0.55f).coerceIn(0f, 1f)
    val u = ((l - 0.55f) / 0.45f).coerceIn(0f, 1f)
    fun mix(amber: Float, gold: Float, heart: Float) = l * ((amber * (1 - t) + gold * t) * (1 - u) + heart * u)
    return floatArrayOf(mix(1f, 1f, 1f), mix(0.46f, 0.66f, 0.92f), mix(0.08f, 0.24f, 0.66f))
}

/**
 * [picture], an overlay of light on black, made the sun's: its beams yellow whatever colour they were, lifted
 * where they are faint, and a glow about their heart, [reach] pixels of it to one of the frame's shorter sides.
 */
internal fun golden(picture: Bitmap, seen: Seen, reach: Float): Bitmap {
    val w = picture.width
    val h = picture.height
    val out = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val lift = min(1f / seen.peak.coerceAtLeast(0.2f), 2.2f) * 0.8f
    val colors = IntArray(256) { i ->
        val c = goldenAt(i / 255f)
        (0xFF shl 24) or ((c[0] * 255).roundToInt() shl 16) or ((c[1] * 255).roundToInt() shl 8) or (c[2] * 255).roundToInt()
    }
    // the glow by the square of the distance, so that no pixel takes a square root
    val steps = 2048
    val far = 6f
    val glow = FloatArray(steps + 1) { i ->
        val d2 = i * far / steps
        (0.6 * exp(-d2 / (0.24 * 0.24)) + 0.26 * exp(-d2 / (0.7 * 0.7))).toFloat()
    }
    val cx = seen.heart * w
    val cy = seen.heartY * h
    val row = IntArray(w)
    for (y in 0 until h) {
        picture.getPixels(row, 0, w, 0, y, w, 1)
        val dy = (y - cy) / reach
        for (x in 0 until w) {
            val p = row[x]
            val light = ((p shr 16 and 255) * 77 + (p shr 8 and 255) * 150 + (p and 255) * 29) / 65280f
            val dx = (x - cx) / reach
            val g = glow[((dx * dx + dy * dy) * steps / far).toInt().coerceAtMost(steps)]
            // half a level of noise keeps the glow from showing steps
            val noise = ((x * 73856093 xor y * 19349663) and 255) / 255f - 0.5f
            val l = 1f - (1f - min(light * lift, 1f)) * (1f - g)
            row[x] = colors[(l * 255f + 0.5f + noise * 0.8f).toInt().coerceIn(0, 255)]
        }
        out.setPixels(row, 0, w, 0, y, w, 1)
    }
    return out
}

/** The warmth that golden light from ([x], [y]) of a frame [w] by [h] gives the picture, to multiply it by. */
internal fun warmth(w: Int, h: Int, x: Float, y: Float): Bitmap {
    val px = IntArray(w * h)
    val reach = min(w, h).toFloat()
    for (j in 0 until h) for (i in 0 until w) {
        val dx = (i - x * w) / reach
        val dy = (j - y * h) / reach
        val near = exp(-(dx * dx + dy * dy) / 0.81f)
        val noise = ((i * 73856093 xor j * 19349663) and 255) / 255f - 0.5f
        fun at(away: Float, close: Float) = ((away * (1 - near) + close * near) * 255f + 0.5f + noise).toInt().coerceIn(0, 255)
        px[j * w + i] = (0xFF shl 24) or (at(0.9f, 1f) shl 16) or (at(0.665f, 0.83f) shl 8) or at(0.43f, 0.55f)
    }
    return Bitmap.createBitmap(px, w, h, Bitmap.Config.ARGB_8888)
}

/** The folders of overlays the user chose: their own files, read where they are. */
internal object OverlayFolders {
    private const val FILE = "capcraft-overlays"

    private fun prefs(context: Context) = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun folder(context: Context): Uri? = prefs(context).getString("folder", null)?.let(Uri::parse)

    fun set(context: Context, tree: Uri?) {
        folder(context)?.let { old -> if (old != tree) runCatching { context.contentResolver.releasePersistableUriPermission(old, Intent.FLAG_GRANT_READ_URI_PERMISSION) } }
        if (tree != null) context.contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        prefs(context).edit().putString("folder", tree?.toString()).apply()
    }

    /** Whether light of the sun's kind is laid golden. */
    fun golden(context: Context): Boolean = prefs(context).getBoolean("golden", true)

    fun setGolden(context: Context, on: Boolean) = prefs(context).edit().putBoolean("golden", on).apply()

    private val PICTURES = setOf("jpg", "jpeg", "png", "webp")

    fun read(context: Context): List<OverlayFile> {
        val tree = folder(context) ?: return emptyList()
        val root = runCatching { DocumentsContract.getTreeDocumentId(tree) }.getOrNull() ?: return emptyList()
        val found = ArrayList<OverlayFile>()
        val todo = ArrayDeque(listOf(Triple(root, "Overlays", 0)))
        while (todo.isNotEmpty() && found.size < 4000) {
            val (id, category, depth) = todo.removeFirst()
            runCatching {
                context.contentResolver.query(
                    DocumentsContract.buildChildDocumentsUriUsingTree(tree, id),
                    arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE),
                    null, null, null,
                )?.use { c ->
                    while (c.moveToNext()) {
                        val name = c.getString(1) ?: continue
                        if (c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR) {
                            if (depth < 4) todo.add(Triple(c.getString(0), soundCategory(name.replace(Regex("\\s*overlays?$", RegexOption.IGNORE_CASE), "")), depth + 1))
                        } else if (name.substringAfterLast('.').lowercase() in PICTURES && !name.startsWith(".")) {
                            found.add(OverlayFile(DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(0)), name.substringBeforeLast('.'), category))
                        }
                    }
                }
            }
        }
        return found.sortedWith(compareBy({ it.category.lowercase() }, { it.name.lowercase() }))
    }

    private val small = LruCache<String, Bitmap>(220)

    /** [file] made small, kept in memory and in the cache. */
    fun thumb(context: Context, file: OverlayFile): Bitmap? {
        val key = file.uri.toString()
        small.get(key)?.let { return it }
        val kept = File(File(context.cacheDir, "overlay-thumbs").apply { mkdirs() }, Integer.toHexString(key.hashCode()) + "-" + key.length + ".jpg")
        val made = BitmapFactory.decodeFile(kept.path) ?: decode(context, file.uri, 220)?.also { b -> runCatching { kept.outputStream().use { b.compress(Bitmap.CompressFormat.JPEG, 85, it) } } }
        if (made != null) small.put(key, made)
        return made
    }

    /** The picture in [uri], its longer side [side] at the most. */
    fun decode(context: Context, uri: Uri, side: Int): Bitmap? = runCatching {
        val size = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, size) }
        var by = 1
        while (max(size.outWidth, size.outHeight) / (by * 2) >= side) by *= 2
        val b = context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = by }) } ?: return null
        val k = side.toFloat() / max(b.width, b.height)
        if (k >= 1f) b else Bitmap.createScaledBitmap(b, (b.width * k).roundToInt().coerceAtLeast(1), (b.height * k).roundToInt().coerceAtLeast(1), true)
    }.getOrNull()
}

/** What the panel last laid, taken off again when another is tried. */
private object Trying {
    var clip: String? = null
    var file by mutableStateOf<OverlayFile?>(null)
    var id by mutableStateOf<String?>(null)
    /** The warmth golden light gives the picture, a layer of its own. */
    var warm by mutableStateOf<String?>(null)
    var strength by mutableFloatStateOf(0.7f)
    var busy by mutableStateOf(false)
    /** The last one laid is behind the people. */
    @Volatile var behind = false
}

private class Laying(val view: TimelineView, val how: Laid, val id: String, val warm: String)

private suspend fun lay(vm: EditorViewModel, context: Context, clip: ClipView, file: OverlayFile) {
    val s = vm.session ?: return
    val t = vm.timeline ?: return
    Trying.busy = true
    val at = vm.playhead.coerceIn(clip.startUs, clip.startUs + clip.durationUs - 1)
    val old = listOfNotNull(Trying.id, Trying.warm).filter { vm.clip(it) != null }
    val preset = presetFor(file.category)
    val sun = preset.golden && OverlayFolders.golden(context)
    val result = withContext(Threads.commands) {
        runCatching {
            if (old.isNotEmpty()) s.delete(old)
            val h = 120
            val w = (h * t.width.toFloat() / t.height.toFloat()).roundToInt().coerceAtLeast(16)
            val scene = s.frame(at, w.toUInt(), h.toUInt(), Motion.STILL).intoBitmap(null)?.let(::sceneOf) ?: Scene(0.4f, 0f, 0f)
            // no larger than a frame wants it, so it stays quick to draw
            val picture = OverlayFolders.decode(context, file.uri, 2560) ?: error("That overlay could not be read.")
            val small = Bitmap.createScaledBitmap(picture, 128, (128f * picture.height / picture.width).roundToInt().coerceAtLeast(1), true)
            val dir = File(vm.path, "media").apply { mkdirs() }
            val out = Media.uniqueFile(dir, Media.safeName(file.name) + ".jpg")
            val frame = t.width.toFloat() / t.height.toFloat()
            val look = seen(small)
            val how = laid(preset, look, scene, frame, sun)
            var warm = ""
            if (sun) {
                // what of the picture the frame shows, its shorter side
                val tall = if (look.aspect >= frame) picture.height.toFloat() else picture.width / frame
                val made = golden(picture, look, min(tall, tall * frame))
                out.outputStream().use { made.compress(Bitmap.CompressFormat.JPEG, 94, it) }
                val fitW = if (look.aspect >= frame) 1f else look.aspect / frame
                val fitH = if (look.aspect >= frame) frame / look.aspect else 1f
                val heart = if (how.flip) 1f - look.heart else look.heart
                val hx = 0.5f + how.x.toFloat() + (heart - 0.5f) * fitW * how.scale.toFloat()
                val hy = 0.5f + how.y.toFloat() + (look.heartY - 0.5f) * fitH * how.scale.toFloat()
                val gw = if (frame >= 1f) 640 else (640 * frame).roundToInt()
                val gh = if (frame >= 1f) (640 / frame).roundToInt() else 640
                val shade = Media.uniqueFile(dir, "Golden light.jpg")
                shade.outputStream().use { warmth(gw, gh, hx, hy).compress(Bitmap.CompressFormat.JPEG, 95, it) }
                warm = s.layOver(clip.id, shade.absolutePath, Laid("multiply", 0.9, 1.01, 0.0, 0.0, 0.0, false, "")).createdId
            } else {
                out.outputStream().use { picture.compress(Bitmap.CompressFormat.JPEG, 92, it) }
            }
            // the people stay in front of the next one tried, where they were of the last
            val stay = Trying.behind
            var view = s.layOver(clip.id, out.absolutePath, how)
            val made = view.createdId
            val copy = bodyOf(vm, clip.id)?.first
            if (stay && copy != null) {
                view = s.restack(copy, 99)
                // the people stand in the same light
                if (warm.isNotEmpty()) view = s.restack(warm, 99)
            }
            Laying(view, how, made, warm)
        }
    }
    Trying.busy = false
    result.onSuccess { done ->
        vm.applyView(done.view)
        Trying.id = done.id.ifEmpty { null }
        Trying.warm = done.warm.ifEmpty { null }
        Trying.file = file
        Trying.strength = done.how.opacity.toFloat()
    }.onFailure { vm.message = it.userMessage }
}

/**
 * Atmosphere: smoke, mist, light, dust and the like from a folder of the user's own overlays, each laid
 * as its kind is - where it belongs, blended, as strong as the picture bears, its light from the side the
 * picture's comes from. The files stay the user's: they are read from the folder and copied into the project.
 */
@Composable
fun AtmospherePanel(vm: EditorViewModel) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val clip = vm.selectedClip?.takeIf { it.kind == "video" || it.kind == "image" } ?: vm.mainClipAtPlayhead()
    var reads by remember { mutableIntStateOf(0) }
    var all by remember { mutableStateOf<List<OverlayFile>?>(null) }
    var category by remember { mutableIntStateOf(0) }
    var finding by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(reads) { all = withContext(Dispatchers.IO) { OverlayFolders.read(context) } }
    DisposableEffect(clip?.id) {
        if (Trying.clip != clip?.id) {
            Trying.clip = clip?.id
            Trying.id = null
            Trying.warm = null
            Trying.file = null
        }
        onDispose {
            Trying.id = null
            Trying.warm = null
            Trying.file = null
            Trying.clip = null
        }
    }
    val choose = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
        if (tree != null) runCatching { OverlayFolders.set(context, tree) }.onSuccess { all = null; category = 0; reads++ }.onFailure { vm.message = "Could not read that folder" }
    }
    val listener = remember {
        object : SpeechListener {
            override fun onProgress(fraction: Float, stage: String) {
                scope.launch(Dispatchers.Main) { finding = "${(fraction * 100).roundToInt()}%" }
            }

            override fun onDone(result: TimelineView?) {
                scope.launch(Dispatchers.Main) {
                    finding = null
                    result?.let { vm.applyView(it) }
                    Trying.warm?.let { w -> if (vm.clip(w) != null) vm.edit { restack(w, 99) } }
                }
            }

            override fun onFailed(message: String) {
                scope.launch(Dispatchers.Main) {
                    finding = null
                    vm.message = message
                }
            }
        }
    }
    // whether the people's copy is in front of the overlay: the boxes come the front one first
    val front by produceState(false, vm.timeline?.revision, Trying.id, clip?.id) {
        val over = Trying.id
        val copy = clip?.let { bodyOf(vm, it.id) }?.first
        val s = vm.session
        val at = clip?.let { vm.playhead.coerceIn(it.startUs, it.startUs + it.durationUs - 1) } ?: 0L
        value = if (over == null || copy == null || s == null) false else withContext(Threads.commands) {
            runCatching { s.clipBoxes(at).map { it.clipId } }.getOrDefault(emptyList()).let { o -> copy in o && over in o && o.indexOf(copy) < o.indexOf(over) }
        }
    }

    if (Trying.id != null) Trying.behind = front
    PanelFrame("Atmosphere", vm, tall = true, titleOnTop = true, left = {
        if (Trying.id != null && !Trying.busy) {
            Text(
                "Take off",
                style = Type.Small.copy(color = Palette.DockText),
                modifier = Modifier.clickable {
                    val id = Trying.id ?: return@clickable
                    vm.edit { delete(listOfNotNull(id, Trying.warm?.takeIf { w -> vm.clip(w) != null })) }
                    Trying.id = null
                    Trying.warm = null
                    Trying.file = null
                }.padding(8.dp),
            )
        }
    }) {
        val list = all
        when {
            OverlayFolders.folder(context) == null -> Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(14.dp))
                Glyph(LucideIcons.Folder, Palette.DockText, size = 32.dp)
                Spacer(Modifier.height(12.dp))
                Text(
                    "Choose a folder of overlays on this phone: smoke, mist, light rays, dust and the like, light on black. The folders in it become categories.",
                    style = Type.Body.copy(color = Palette.Text, textAlign = TextAlign.Center),
                )
                Spacer(Modifier.height(18.dp))
                WideButton("Choose folder") { choose.launch(shelf("Pictures")) }
            }
            list == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("Reading your overlays", style = Type.Body.copy(color = Palette.TextSecondary)) }
            list.isEmpty() -> Column(Modifier.fillMaxSize().padding(24.dp)) {
                Text("There are no pictures in that folder.", style = Type.Body.copy(color = Palette.TextSecondary))
                Spacer(Modifier.height(14.dp))
                WideButton("Choose another") { choose.launch(shelf("Pictures")) }
            }
            else -> {
                val categories = remember(list) { list.map { it.category }.distinct() }
                Tabs(categories, category) { category = it }
                if (Trying.id != null && vm.clip(Trying.id!!) != null) {
                    ValueSlider(
                        value = Trying.strength * 100f, range = 0f..100f, label = { "${it.roundToInt()}" },
                        onChange = { v ->
                            Trying.strength = v / 100f
                            Trying.id?.let { vm.patch(it, JSONObject().put("opacity", (v / 100f).toDouble()), "atmosphere") }
                            Trying.warm?.takeIf { vm.clip(it) != null }?.let { vm.patch(it, JSONObject().put("opacity", min(1f, v / 100f * 1.1f).toDouble()), "atmosphere") }
                        },
                        onDone = { vm.endGesture() }, title = "Strength",
                    )
                    Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        val now = Trying.file
                        if (now != null && presetFor(now.category).golden) {
                            val sun = Trying.warm != null
                            Choice("Golden", sun) {
                                if (clip != null && !Trying.busy) {
                                    OverlayFolders.setGolden(context, !sun)
                                    scope.launch { lay(vm, context, clip, now) }
                                }
                            }
                        }
                        Choice("Mirror", false) {
                            val id = Trying.id ?: return@Choice
                            val now = vm.clipJson(id)?.optBoolean("flipH") ?: false
                            vm.patch(id, JSONObject().put("flipH", !now))
                        }
                        Choice(if (finding != null) "Finding people ${finding}" else "Behind people", front) {
                            val s = vm.session ?: return@Choice
                            val c = clip ?: return@Choice
                            if (finding != null) return@Choice
                            val over = Trying.id ?: return@Choice
                            val body = bodyOf(vm, c.id)
                            when {
                                // the people are cut out already, for their light or an effect: one or the other goes to the front
                                body != null && !front -> {
                                    vm.edit { restack(body.first, 99) }
                                    Trying.warm?.let { w -> if (vm.clip(w) != null) vm.edit { restack(w, 99) } }
                                }
                                body != null && body.second != "front" -> vm.edit { restack(over, 99) }
                                else -> {
                                    finding = ""
                                    scope.launch(Threads.commands) {
                                        runCatching { s.bodyEffect(c.id, if (front) "" else "front", listener) }.onFailure { e -> withContext(Dispatchers.Main) { finding = null; vm.message = e.userMessage } }
                                    }
                                }
                            }
                        }
                        Spacer(Modifier.weight(1f))
                        Choice("Keep", false) {
                            // the next one goes over this one rather than in its place
                            Trying.id = null
                            Trying.warm = null
                            Trying.file = null
                        }
                    }
                }
                val shown = list.filter { it.category == categories.getOrNull(category) }
                LazyVerticalGrid(
                    GridCells.Fixed(4),
                    Modifier.fillMaxWidth().weight(1f),
                    contentPadding = PaddingValues(12.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(shown, key = { it.uri.toString() }) { f ->
                        Thumb(f, f.uri == Trying.file?.uri, Trying.busy && f.uri == Trying.file?.uri) {
                            if (clip == null) vm.message = "Add a clip first"
                            else if (!Trying.busy) scope.launch { lay(vm, context, clip, f) }
                        }
                    }
                    item { Box(Modifier.aspectRatio(1f).clip(RoundedCornerShape(8.dp)).background(Color(0xFF2E2E2E)).clickable { choose.launch(shelf("Pictures")) }, contentAlignment = Alignment.Center) { Glyph(LucideIcons.FolderOpen, Palette.DockText, size = 22.dp, description = "Another folder") } }
                }
            }
        }
    }
}

@Composable
private fun Choice(label: String, on: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.height(32.dp).clip(RoundedCornerShape(16.dp)).background(if (on) Palette.Accent else Color(0xFF3A3A3A)).clickable(onClick = onClick).padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) { Text(label, style = Type.Small.copy(fontSize = 13.sp, color = if (on) Palette.OnAccent else Palette.Text)) }
}

@Composable
private fun Thumb(file: OverlayFile, on: Boolean, busy: Boolean, onClick: () -> Unit) {
    val context = LocalContext.current
    val picture by produceState<Bitmap?>(null, file.uri) { value = withContext(Dispatchers.IO) { OverlayFolders.thumb(context, file) } }
    Box(
        Modifier.aspectRatio(1f).clip(RoundedCornerShape(8.dp)).background(Color.Black)
            .border(2.dp, if (on) Palette.Accent else Color(0xFF2E2E2E), RoundedCornerShape(8.dp)).clickable(onClick = onClick),
    ) {
        picture?.let { Image(it.asImageBitmap(), contentDescription = file.name, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
        if (busy) Box(Modifier.fillMaxSize().background(Color(0x88000000)))
    }
}
