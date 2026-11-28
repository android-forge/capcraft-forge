package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.engine.Media
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.common.MediaKinds
import io.github.artcraftandroid.capcraft.ui.common.MediaPick
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.unit.dp
import io.github.artcraftandroid.capcraft.core.MobileSession
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import org.json.JSONArray
import org.json.JSONObject

/** Where the timeline keeps a collage: its pictures' clip ids in cell order, and the layout chosen. */
private const val COLLAGE_KEY = "capcraftCollage"

/** One cell of a collage: where it sits on the frame, in fractions of the frame's width and height. */
internal class Cell(val x: Float, val y: Float, val w: Float, val h: Float, val rot: Float = 0f, val shape: String = "")

/**
 * A collage as the timeline keeps it: its pictures in cell order, the layout, and its borders - the
 * gap between cells and round the edge (a fraction of the frame's shorter side), how round the cells'
 * corners are (0 to 1), and the colour that shows in the gaps.
 */
internal data class Collage(
    val ids: List<String>,
    val layout: Int,
    val gap: Float = 0f,
    val radius: Float = 0f,
    val color: String = "#ffffff",
    /** A frame of Forge's own (see `CollageStyles.kt`), or empty for the plain grid. */
    val style: String = "",
    /** Each picture's zoom in its space and where its middle is moved to, by clip id. */
    val adjust: Map<String, Triple<Float, Float, Float>> = emptyMap(),
)

/** The frame shapes a collage can take, as the canvas sizes they set. */
internal val COLLAGE_FRAMES = listOf("1:1" to (1080 to 1080), "4:5" to (1080 to 1350), "3:4" to (1080 to 1440), "9:16" to (1080 to 1920), "16:9" to (1920 to 1080))

/** Colours for the gaps. */

/** Rows of [counts] cells each, the rows equally tall and each row's cells equally wide. */
private fun rows(vararg counts: Int): List<Cell> = counts.withIndex().flatMap { (r, n) ->
    (0 until n).map { c -> Cell(c / n.toFloat(), r / counts.size.toFloat(), 1f / n, 1f / counts.size) }
}

/** [rows] turned on its side: columns of [counts] cells each. */
private fun columns(vararg counts: Int): List<Cell> = rows(*counts).map { Cell(it.y, it.x, it.h, it.w) }

/**
 * One big cell taking [share] of the frame along one [side] (0 left, 1 top, 2 right, 3 bottom), and
 * [rest] cells side by side in what is left.
 */
private fun featured(rest: Int, share: Float, side: Int): List<Cell> {
    val others = (0 until rest).map { i ->
        val f = i / rest.toFloat()
        val g = 1f / rest
        when (side) {
            0 -> Cell(share, f, 1 - share, g)
            1 -> Cell(f, share, g, 1 - share)
            2 -> Cell(0f, f, 1 - share, g)
            else -> Cell(f, 0f, g, 1 - share)
        }
    }
    val big = when (side) {
        0 -> Cell(0f, 0f, share, 1f)
        1 -> Cell(0f, 0f, 1f, share)
        2 -> Cell(1 - share, 0f, share, 1f)
        else -> Cell(0f, 1 - share, 1f, share)
    }
    return listOf(big) + others
}

/** The layouts for [n] pictures, the first the one a new collage starts with. */
internal fun collageLayouts(n: Int): List<List<Cell>> = when (n) {
    1 -> listOf(rows(1))
    2 -> listOf(rows(1, 1), rows(2), featured(1, 2 / 3f, 0), featured(1, 2 / 3f, 1), featured(1, 2 / 3f, 2), featured(1, 2 / 3f, 3))
    3 -> listOf(rows(1, 1, 1), rows(3), featured(2, 0.5f, 0), featured(2, 0.5f, 1), featured(2, 0.5f, 2), featured(2, 0.5f, 3))
    4 -> listOf(rows(2, 2), rows(4), rows(1, 1, 1, 1), featured(3, 0.6f, 0), featured(3, 0.6f, 1), rows(1, 3), rows(3, 1))
    5 -> listOf(rows(2, 3), rows(3, 2), columns(2, 3), columns(3, 2), rows(1, 4), featured(4, 0.5f, 1))
    6 -> listOf(rows(3, 3), rows(2, 2, 2), rows(1, 2, 3), featured(5, 0.5f, 1), columns(1, 2, 3))
    7 -> listOf(rows(3, 4), rows(4, 3), rows(2, 3, 2), columns(3, 4), columns(2, 3, 2))
    8 -> listOf(rows(4, 4), rows(2, 2, 2, 2), rows(3, 2, 3), columns(3, 2, 3))
    else -> listOf(rows(3, 3, 3), rows(4, 1, 4), columns(4, 1, 4))
}

/** The collage kept on this session's timeline, if it is one. */
internal fun MobileSession.collage(): Collage? {
    val kept = runCatching { extra(COLLAGE_KEY).takeIf { it.isNotBlank() }?.let(::JSONObject) }.getOrNull() ?: return null
    val ids = kept.optJSONArray("ids") ?: return null
    return Collage(
        (0 until ids.length()).map { ids.optString(it) },
        kept.optInt("layout", 0),
        kept.optDouble("gap", 0.0).toFloat(),
        kept.optDouble("radius", 0.0).toFloat(),
        kept.optString("color", "#ffffff"),
        kept.optString("style", ""),
        kept.optJSONObject("adjust")?.let { a ->
            a.keys().asSequence().associateWith { k ->
                val v = a.optJSONArray(k)
                Triple(v?.optDouble(0, 1.0)?.toFloat() ?: 1f, v?.optDouble(1, 0.0)?.toFloat() ?: 0f, v?.optDouble(2, 0.0)?.toFloat() ?: 0f)
            }
        }.orEmpty(),
    )
}

/** Where [c]'s pictures go on a [fw] x [fh] frame, as the collage lays them. */
internal fun collageCells(c: Collage, fw: Int, fh: Int, styled: Boolean = c.style.isNotEmpty()): List<Cell>? =
    if (styled) styleCells(c.style, c.ids.size, fw, fh)
    else collageLayouts(c.ids.size).getOrNull(c.layout.coerceIn(0, (collageLayouts(c.ids.size).size - 1).coerceAtLeast(0)))
        ?.map { inset(it, c.gap, fw.toDouble(), fh.toDouble()) }

/**
 * [cell] with its share of the gap taken off. The layout is first drawn in from the frame's edge by
 * half a gap, then every cell loses half a gap on every side: the gap is the same between cells as at
 * the edge, and cells of a size stay of a size.
 */
private fun inset(cell: Cell, gap: Float, frameW: Double, frameH: Double): Cell {
    if (gap <= 0f) return cell
    val half = gap * minOf(frameW, frameH) / 2
    val (mx, my) = (half / frameW) to (half / frameH)
    val x = mx + cell.x * (1 - 2 * mx)
    val y = my + cell.y * (1 - 2 * my)
    val w = cell.w * (1 - 2 * mx)
    val h = cell.h * (1 - 2 * my)
    return Cell((x + mx).toFloat(), (y + my).toFloat(), (w - 2 * mx).toFloat().coerceAtLeast(0.02f), (h - 2 * my).toFloat().coerceAtLeast(0.02f))
}

/**
 * Lays [ids]'s pictures into the cells of layout [layout] on [view]'s frame, as one step: each
 * cropped to its cell's shape about its middle and scaled to fill it, with the collage kept on the
 * timeline. A picture no longer there leaves its cell empty. The view after.
 */
internal fun MobileSession.layCollage(view: TimelineView, ids: List<String>, layout: Int): TimelineView =
    layCollage(view, (collage() ?: Collage(ids, layout)).copy(ids = ids, layout = layout))

/**
 * Lays [c]'s pictures into its layout's cells, less the gaps, each cut to its cell and its corners
 * rounded by a mask; the gaps show the background, set to [c]'s colour. One step; the view after.
 */
internal fun MobileSession.layCollage(view0: TimelineView, c: Collage, mediaDir: java.io.File? = null, gesture: String? = null): TimelineView {
    val ids = c.ids
    val layout = c.layout.coerceIn(0, (collageLayouts(ids.size).size - 1).coerceAtLeast(0))
    val (fw, fh) = view0.width.toInt() to view0.height.toInt()
    val view = view0
    val styled = c.style.isNotEmpty() && mediaDir != null
    val cells = collageCells(c, fw, fh, styled) ?: return view0
    val batch = JSONArray()
    // The background, in the same step as the pictures, and only when the collage changes it: a
    // frame of our own draws its own (for this count and size); the grid's gap colour is set when
    // it is picked, so a background chosen in the editor stays through a re-lay.
    val before = collage()
    val ground: JSONObject? = when {
        styled -> {
            val path = styleBackground(c.style, ids.size, fw, fh, mediaDir!!)
            JSONObject().put("kind", "image").put("path", path).takeIf { !background().contains(path) }
        }
        before == null || before.style != c.style || before.color != c.color -> JSONObject().put("kind", "color").put("color", c.color)
        else -> null
    }
    if (ground != null) {
        batch.put(JSONObject().put("op", "setTimelineExtra").put("key", "capcraftBackground").put("value", ground))
        batch.put(JSONObject().put("op", "setTimelineExtra").put("key", "capcraftBackgrounds").put("value", JSONObject.NULL))
    }
    val frameW = view.width.toDouble()
    val frameH = view.height.toDouble()
    val clips = view.rows.flatMap { it.clips }
    ids.zip(cells).forEach { (id, cell) ->
        val clip = clips.firstOrNull { it.id == id } ?: return@forEach
        val mw = clip.mediaWidth.toDouble().takeIf { it > 0 } ?: frameW
        val mh = clip.mediaHeight.toDouble().takeIf { it > 0 } ?: frameH
        val cellW = cell.w * frameW
        val cellH = cell.h * frameH
        // cut to the cell's shape, evenly off both sides that are too long
        var left = 0.0
        var top = 0.0
        if (mw / mh > cellW / cellH) left = (1 - (cellW / cellH) / (mw / mh)) / 2 else top = (1 - (mw / mh) / (cellW / cellH)) / 2
        // zoomed and moved within its space: a smaller window, its middle shifted, kept on the picture
        val (zoom, ox, oy) = c.adjust[id] ?: Triple(1f, 0f, 0f)
        val z = zoom.coerceIn(1f, 6f).toDouble()
        val winW = (1 - 2 * left) / z
        val winH = (1 - 2 * top) / z
        val cx = (0.5 + ox).coerceIn(winW / 2, 1 - winW / 2)
        val cy = (0.5 + oy).coerceIn(winH / 2, 1 - winH / 2)
        val cropL = cx - winW / 2
        val cropR = 1 - cx - winW / 2
        val cropT = cy - winH / 2
        val cropB = 1 - cy - winH / 2
        val cutW = mw * winW
        val cutH = mh * winH
        // the engine fits the cut picture to the frame; the scale takes it from there to the cell
        val fit = minOf(frameW / cutW, frameH / cutH)
        // rounded corners: a rectangle mask the cell's shape, over the whole cut picture
        val aspect = cellW / cellH
        val box = JSONObject().put("x", 0.0).put("y", 0.0).put("rotation", 0.0).put("feather", 0.0)
        val mask = when (cell.shape) {
            "circle", "heart", "star" -> box.put("shape", cell.shape).put("width", 1.0).put("height", 1.0)
            "round" -> box.put("shape", "rectangle").put("width", maxOf(aspect, 1.0)).put("height", maxOf(1 / aspect, 1.0)).put("roundness", 0.45)
            else -> if (!styled && c.radius > 0f) {
                box.put("shape", "rectangle").put("width", maxOf(aspect, 1.0)).put("height", maxOf(1 / aspect, 1.0)).put("roundness", c.radius.toDouble())
            } else JSONObject.NULL
        }
        batch.put(
            JSONObject().put("op", "updateClip").put("clipId", id).put(
                "patch",
                JSONObject().put("crop", JSONObject().put("left", cropL).put("right", cropR).put("top", cropT).put("bottom", cropB))
                    .put("mask", mask),
            ),
        )
        batch.put(
            JSONObject().put("op", "setClipTransform").put("clipId", id)
                .put("scale", cellW / (cutW * fit))
                .put("offsetX", cell.x + cell.w / 2 - 0.5)
                .put("offsetY", cell.y + cell.h / 2 - 0.5)
                .put("rotation", cell.rot.toDouble()),
        )
    }
    batch.put(
        JSONObject().put("op", "setTimelineExtra").put("key", COLLAGE_KEY)
            .put(
                "value",
                JSONObject().put("ids", JSONArray(ids)).put("layout", layout).put("gap", c.gap.toDouble())
                    .put("radius", c.radius.toDouble()).put("color", c.color).put("style", c.style)
                    .put("adjust", JSONObject(c.adjust.mapValues { (_, v) -> JSONArray(listOf(v.first.toDouble(), v.second.toDouble(), v.third.toDouble())) })),
            ),
    )
    return apply(JSONObject().put("op", "batch").put("commands", batch).toString(), gesture)
}

/** A name for one collage change: every edit made under it is one undo step. */
internal fun collageGesture(): String = "collage-${System.nanoTime()}"

/**
 * The collage's own tools, in three tabs. Layout: how many pictures (a + picks one more, a - takes the
 * last off), the layouts for that many, and the current one as a map - tap two cells to swap their
 * pictures. Borders: the gap's width, the corners' roundness and the gap's colour. Frame: the shape.
 */
@Composable
fun CollagePanel(vm: EditorViewModel) {
    if (remember(vm.timeline?.revision) { vm.session?.collage() } == null) {
        vm.panel = null
        return
    }
    PanelFrame("Collage", vm, height = 250.dp, titleOnTop = true) { CollageTools(vm) }
}

/** The collage's tabs, for its panel in the editor and for its own screen. */
@Composable
fun CollageTools(vm: EditorViewModel) {
    // a picture picked in another collage is not one picked here
    androidx.compose.runtime.LaunchedEffect(vm.path) { CollagePick.id = null }
    val kept = remember(vm.timeline?.revision) { vm.session?.collage() } ?: return
    var tab by remember { mutableStateOf(0) }
    var swapFrom by remember { mutableStateOf<Int?>(null) }
    val layouts = collageLayouts(kept.ids.size)
    val media = java.io.File(vm.path, "media")
    fun lay(next: Collage) = vm.edit { layCollage(view(), next, media, collageGesture()).also { endGesture() } }
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("Layout", "Frames", "Borders", "Size").forEachIndexed { i, name -> Pill(name, tab == i) { tab = i } }
        }
        when (tab) {
            0 -> Column(Modifier.fillMaxWidth().padding(top = 10.dp)) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Pictures", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(64.dp))
                    Pill("", false, LucideIcons.Minus, "Remove the last picture", choice = false) {
                        if (kept.ids.size > 2) {
                            val last = kept.ids.last()
                            vm.edit {
                                // the picture off and the rest re-laid, one step
                                val g = collageGesture()
                                val gone = apply(JSONObject().put("op", "removeClips").put("clipIds", JSONArray(listOf(last))).put("ripple", false).toString(), g)
                                layCollage(gone, kept.copy(ids = kept.ids.dropLast(1), layout = 0, adjust = kept.adjust - last), media, g).also { endGesture() }
                            }
                        } else vm.message = "A collage keeps at least two pictures"
                    }
                    Text("${kept.ids.size}", style = Type.BodyStrong.copy(color = Palette.Text), modifier = Modifier.padding(horizontal = 12.dp))
                    Pill("", false, LucideIcons.Plus, "Add a picture", choice = false) {
                        if (kept.ids.size >= 9) vm.message = "A collage holds up to nine pictures"
                        else MediaPick.open(MediaKinds.BOTH, multiple = false) { uris ->
                            val uri = uris.firstOrNull() ?: return@open
                            val app = vm.getApplication<android.app.Application>()
                            vm.edit {
                                val picked = import(Media.importUri(app, vm.path, uri).absolutePath)
                                // a lane of its own at the top, the picture on it and every picture re-laid, one step
                                val g = collageGesture()
                                val lane = apply(JSONObject().put("op", "insertTrack").put("index", 10_000).toString(), g)
                                val added = apply(
                                    JSONObject().put("op", "addClip").put("mediaId", picked.id).put("trackId", lane.createdId).put("start", 0.0).put("ripple", false).toString(), g,
                                )
                                layCollage(added, kept.copy(ids = kept.ids + added.createdId, layout = 0), media, g).also { endGesture() }
                            }
                        }
                    }
                    Box(Modifier.weight(1f))
                    // the current layout as a map: two taps swap two pictures
                    CellMap(layouts.getOrNull(kept.layout) ?: layouts.first(), swapFrom) { i ->
                        val from = swapFrom
                        if (from == null) swapFrom = i
                        else {
                            swapFrom = null
                            if (from != i) {
                                val ids = kept.ids.toMutableList()
                                ids[from] = ids[i].also { ids[i] = ids[from] }
                                lay(kept.copy(ids = ids))
                            }
                        }
                    }
                }
                // the tapped picture's zoom, without a pinch
                val picked = CollagePick.id?.takeIf { it in kept.ids }
                if (picked != null) {
                    val (z, ox, oy) = kept.adjust[picked] ?: Triple(1f, 0f, 0f)
                    var zoomGesture by remember(picked) { mutableStateOf(collageGesture()) }
                    Row(Modifier.fillMaxWidth().padding(end = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.weight(1f)) {
                            ValueSlider(z, 1f..6f, { "%.1f×".format(it) }, { v ->
                                vm.edit { layCollage(view(), kept.copy(adjust = kept.adjust + (picked to Triple(v, ox, oy))), media, zoomGesture) }
                            }, { vm.endGesture(); zoomGesture = collageGesture() }, title = "Picture ${kept.ids.indexOf(picked) + 1} zoom")
                        }
                        Pill("Reset", false, choice = false) { lay(kept.copy(adjust = kept.adjust - picked)) }
                    }
                }
                Text("Tap a picture to zoom it with a slider, or pinch it; drag to move it, double-tap to reset", style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 12.sp), modifier = Modifier.padding(horizontal = 12.dp, vertical = 2.dp))
                if (swapFrom != null) Text("Tap another cell to swap", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.padding(horizontal = 12.dp))
                LazyRow(Modifier.fillMaxWidth().padding(top = 8.dp), contentPadding = PaddingValues(horizontal = 10.dp)) {
                    items(layouts.size) { i -> LayoutTile(layouts[i], kept.style.isEmpty() && i == kept.layout) { lay(kept.copy(layout = i, style = "")) } }
                }
            }
            1 -> Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                COLLAGE_STYLES.forEach { st ->
                    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.selectable(selected = st.id == kept.style, role = Role.Button) { lay(kept.copy(style = st.id)) }) {
                        StyleTile(st.id, kept.ids.size, st.id == kept.style)
                        Text(st.name, style = Type.Small.copy(color = if (st.id == kept.style) Palette.Accent else Palette.Text, fontSize = 12.sp), modifier = Modifier.padding(top = 4.dp))
                    }
                }
            }
            2 -> Column(Modifier.fillMaxWidth().padding(top = 6.dp)) {
                ValueSlider(kept.gap * 1000f, 0f..60f, { "%.0f".format(it) }, { v -> lay(kept.copy(gap = v / 1000f)) }, { vm.endGesture() }, title = "Width")
                ValueSlider(kept.radius * 100f, 0f..100f, { "%.0f".format(it) }, { v -> lay(kept.copy(radius = v / 100f)) }, { vm.endGesture() }, title = "Radius")
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    PICK_COLOURS.forEach { hex ->
                        val on = hex.equals(kept.color, ignoreCase = true)
                        Box(
                            Modifier.minimumInteractiveComponentSize()
                                .semantics { contentDescription = "Gap colour ${colourName(hex).lowercase()}" }
                                .selectable(selected = on, role = Role.RadioButton) { lay(kept.copy(color = hex)) },
                            contentAlignment = Alignment.Center,
                        ) {
                            Box(
                                Modifier.size(30.dp).clip(CircleShape).background(if (on) Palette.Accent else Color.Transparent).padding(3.dp)
                                    .clip(CircleShape).background(Color(android.graphics.Color.parseColor(hex))),
                            )
                        }
                    }
                }
            }
            else -> Row(Modifier.fillMaxWidth().padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val t = vm.timeline
                COLLAGE_FRAMES.forEach { (name, size) ->
                    val on = t != null && t.width.toInt() * size.second == t.height.toInt() * size.first
                    Pill(name, on) {
                        vm.edit {
                            // the new frame and the pictures re-laid on it, one step
                            val g = collageGesture()
                            layCollage(setCanvasWithin(size.first.toUInt(), size.second.toUInt(), g), kept, media, g).also { endGesture() }
                        }
                    }
                }
            }
        }
    }
}

/** A frame drawn small: its cells in their shapes on a swatch of its ground. */
@Composable
private fun StyleTile(style: String, n: Int, on: Boolean) {
    val ground = mapOf("" to 0xFFFFFFFF, "film" to 0xFF111111, "prints" to 0xFFEFE6D8, "love" to 0xFFFFB3C6, "shapes" to 0xFFB7C4FC, "party" to 0xFFFFF4D6, "floral" to 0xFFEAF6EF, "classic" to 0xFFFFFFFF)
    Canvas(Modifier.size(56.dp).clip(RoundedCornerShape(6.dp)).background(Color(ground[style] ?: 0xFFFFFFFF)).then(if (on) Modifier.background(Color.Transparent) else Modifier)) {
        val cells = if (style.isEmpty()) collageLayouts(n).first() else styleCells(style, n, 200, 200)
        val ink = if (style == "film") Color(0xFFCCCCCC) else Color(0xFF8A8A8E)
        cells.forEach { c ->
            val at = Offset(c.x * size.width, c.y * size.height)
            val box = Size(c.w * size.width, c.h * size.height)
            rotate(c.rot, Offset(at.x + box.width / 2, at.y + box.height / 2)) {
                when (c.shape) {
                    "circle", "heart", "star" -> drawCircle(ink, minOf(box.width, box.height) / 2, Offset(at.x + box.width / 2, at.y + box.height / 2))
                    else -> drawRect(ink, at, box)
                }
            }
            if (style.isEmpty()) drawRect(Color.White, at, box, style = Stroke(1.dp.toPx()))
        }
        if (on) drawRect(Palette.Accent, Offset.Zero, size, style = Stroke(3.dp.toPx()))
    }
}

/** A rounded label to tap, lit when [on]. */
@Composable
private fun Pill(label: String, on: Boolean, icon: androidx.compose.ui.graphics.vector.ImageVector? = null, description: String? = null, choice: Boolean = true, onClick: () -> Unit) {
    Box(
        Modifier.minimumInteractiveComponentSize().clip(RoundedCornerShape(16.dp)).background(if (on) Palette.Accent else Palette.DockBack)
            .then(if (choice) Modifier.selectable(selected = on, role = Role.Button, onClick = onClick) else Modifier.clickable(role = Role.Button, onClick = onClick))
            .padding(horizontal = 14.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        if (icon != null) androidx.compose.material3.Icon(icon, contentDescription = description, tint = Palette.Text, modifier = Modifier.size(18.dp))
        else Text(label, style = Type.Small.copy(color = if (on) Palette.OnAccent else Palette.Text, fontSize = 13.sp))
    }
}

/** The picture tapped on the collage screen, for its zoom slider. */
internal object CollagePick {
    var id by mutableStateOf<String?>(null)
}

/** The current layout's cells, numbered; the one picked for a swap lit. */
@Composable
private fun CellMap(cells: List<Cell>, picked: Int?, onTap: (Int) -> Unit) {
    Canvas(
        Modifier.size(96.dp).semantics { contentDescription = "Swap two pictures: tap one cell, then another" }.pointerInput(cells) {
            detectTapGestures { p ->
                val i = cells.indexOfFirst { c -> p.x / size.width in c.x..(c.x + c.w) && p.y / size.height in c.y..(c.y + c.h) }
                if (i >= 0) onTap(i)
            }
        },
    ) {
        val gap = 2.dp.toPx()
        cells.forEachIndexed { i, c ->
            val at = Offset(c.x * size.width + gap / 2, c.y * size.height + gap / 2)
            val box = Size(c.w * size.width - gap, c.h * size.height - gap)
            drawRect(if (i == picked) Palette.Accent else Palette.DockBack, at, box)
        }
    }
}

/** A layout drawn as its cells: outlined, and filled and cyan-edged when chosen. */
@Composable
private fun LayoutTile(cells: List<Cell>, selected: Boolean, onClick: () -> Unit) {
    Canvas(Modifier.padding(horizontal = 9.dp).size(48.dp).semantics { contentDescription = "Layout of ${cells.size}" }.selectable(selected = selected, role = Role.Button, onClick = onClick)) {
        val gap = 1.5.dp.toPx()
        val edge = if (selected) Palette.Accent else Color(0xFFDDDDDD)
        cells.forEach { c ->
            val at = Offset(c.x * size.width + gap / 2, c.y * size.height + gap / 2)
            val box = Size(c.w * size.width - gap, c.h * size.height - gap)
            if (selected) drawRect(Color(0xFF2E5552), at, box)
            drawRect(edge, at, box, style = Stroke(1.5.dp.toPx()))
        }
    }
}

/**
 * Over the collage screen's picture: pinch a picture to zoom it in its space and drag to move it there,
 * double-tap to put it back. The picture is found by where the gesture starts.
 */
@Composable
fun CollageAdjust(vm: EditorViewModel, modifier: Modifier) {
    val kept by androidx.compose.runtime.rememberUpdatedState(remember(vm.timeline?.revision) { vm.session?.collage() })
    val media = remember { java.io.File(vm.path, "media") }
    Box(
        modifier.drawBehind {
            // the picked picture outlined, so it is clear which the slider moves
            val c = kept ?: return@drawBehind
            val t = vm.timeline ?: return@drawBehind
            val id = CollagePick.id?.takeIf { it in c.ids } ?: return@drawBehind
            val fa = t.width.toFloat() / t.height.toFloat()
            val (w, h) = if (size.width / size.height > fa) size.height * fa to size.height else size.width to size.width / fa
            val (l, top) = (size.width - w) / 2 to (size.height - h) / 2
            val cell = collageCells(c, t.width.toInt(), t.height.toInt())?.getOrNull(c.ids.indexOf(id)) ?: return@drawBehind
            drawRect(Palette.Accent, Offset(l + cell.x * w, top + cell.y * h), Size(cell.w * w, cell.h * h), style = Stroke(2.dp.toPx()))
        }.pointerInput(Unit) {
            // the frame as the preview fits it, centred in this box
            fun frame(): androidx.compose.ui.geometry.Rect? {
                val t = vm.timeline ?: return null
                val fa = t.width.toFloat() / t.height.toFloat()
                val (w, h) = if (size.width / size.height.toFloat() > fa) size.height * fa to size.height.toFloat() else size.width.toFloat() to size.width / fa
                return androidx.compose.ui.geometry.Rect(Offset((size.width - w) / 2, (size.height - h) / 2), Size(w, h))
            }
            fun cellAt(p: Offset): Pair<Int, androidx.compose.ui.geometry.Rect>? {
                val c = kept ?: return null
                val t = vm.timeline ?: return null
                val f = frame() ?: return null
                val cells = collageCells(c, t.width.toInt(), t.height.toInt()) ?: return null
                val rects = cells.map { cell -> androidx.compose.ui.geometry.Rect(Offset(f.left + cell.x * f.width, f.top + cell.y * f.height), Size(cell.w * f.width, cell.h * f.height)) }
                val i = rects.indexOfLast { it.contains(p) }
                return if (i < 0) null else i to rects[i]
            }
            var lastTap = 0L
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                val c0 = kept ?: return@awaitEachGesture
                val (index, rect) = cellAt(down.position) ?: return@awaitEachGesture
                val id = c0.ids.getOrNull(index) ?: return@awaitEachGesture
                var (zoom, ox, oy) = c0.adjust[id] ?: Triple(1f, 0f, 0f)
                // how much of the picture the space shows each way at zoom one: for turning a drag into a move
                val clip = vm.clip(id)
                val aspect = rect.width / rect.height
                val mediaAspect = (clip?.mediaWidth?.toFloat() ?: 1f) / (clip?.mediaHeight?.toFloat()?.coerceAtLeast(1f) ?: 1f)
                val shownW = if (mediaAspect > aspect) aspect / mediaAspect else 1f
                val shownH = if (mediaAspect > aspect) 1f else mediaAspect / aspect
                var moved = false
                var sent = 0L
                // the whole pinch is one undo step
                val g = collageGesture()
                while (true) {
                    val event = awaitPointerEvent()
                    if (event.changes.none { it.pressed }) break
                    val z = event.calculateZoom()
                    val pan = event.calculatePan()
                    if (z == 1f && pan == Offset.Zero) continue
                    moved = true
                    zoom = (zoom * z).coerceIn(1f, 6f)
                    ox -= pan.x / rect.width * shownW / zoom
                    oy -= pan.y / rect.height * shownH / zoom
                    event.changes.forEach { it.consume() }
                    val now = System.currentTimeMillis()
                    if (now - sent > 60) {
                        sent = now
                        val next = c0.copy(adjust = c0.adjust + (id to Triple(zoom, ox, oy)))
                        vm.edit { layCollage(view(), next, media, g) }
                    }
                }
                if (moved) {
                    val next = c0.copy(adjust = c0.adjust + (id to Triple(zoom, ox, oy)))
                    vm.edit { layCollage(view(), next, media, g).also { endGesture() } }
                } else {
                    CollagePick.id = id
                    // a second tap soon after the first puts the picture back as it was laid
                    val now = System.currentTimeMillis()
                    if (now - lastTap < 350 && c0.adjust.containsKey(id)) {
                        vm.edit { layCollage(view(), c0.copy(adjust = c0.adjust - id), media) }
                        lastTap = 0L
                    } else lastTap = now
                }
            }
        },
    )
}
