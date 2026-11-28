package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.Canvas
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
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.detectTapGestures
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
internal class Cell(val x: Float, val y: Float, val w: Float, val h: Float)

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
)

/** The frame shapes a collage can take, as the canvas sizes they set. */
internal val COLLAGE_FRAMES = listOf("1:1" to (1080 to 1080), "4:5" to (1080 to 1350), "3:4" to (1080 to 1440), "9:16" to (1080 to 1920), "16:9" to (1920 to 1080))

/** Colours for the gaps. */
private val GAP_COLOURS = listOf("#ffffff", "#000000", "#f4e9d8", "#ffd60a", "#ff9230", "#ee2222", "#ff4fa3", "#7c5cff", "#0a84ff", "#34c759", "#9ad7c4", "#2e2e30")

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
    )
}

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
internal fun MobileSession.layCollage(view0: TimelineView, c: Collage): TimelineView {
    val ids = c.ids
    val layout = c.layout.coerceIn(0, (collageLayouts(ids.size).size - 1).coerceAtLeast(0))
    val view = setBackground(JSONObject().put("kind", "color").put("color", c.color).toString())
    val cells = collageLayouts(ids.size).getOrNull(layout)?.map { inset(it, c.gap, view.width.toDouble(), view.height.toDouble()) } ?: return view0
    val frameW = view.width.toDouble()
    val frameH = view.height.toDouble()
    val clips = view.rows.flatMap { it.clips }
    val batch = JSONArray()
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
        val cutW = mw * (1 - 2 * left)
        val cutH = mh * (1 - 2 * top)
        // the engine fits the cut picture to the frame; the scale takes it from there to the cell
        val fit = minOf(frameW / cutW, frameH / cutH)
        // rounded corners: a rectangle mask the cell's shape, over the whole cut picture
        val aspect = cellW / cellH
        val mask = if (c.radius > 0f) {
            JSONObject().put("shape", "rectangle").put("x", 0.0).put("y", 0.0)
                .put("width", maxOf(aspect, 1.0)).put("height", maxOf(1 / aspect, 1.0))
                .put("rotation", 0.0).put("roundness", c.radius.toDouble()).put("feather", 0.0)
        } else JSONObject.NULL
        batch.put(
            JSONObject().put("op", "updateClip").put("clipId", id).put(
                "patch",
                JSONObject().put("crop", JSONObject().put("left", left).put("right", left).put("top", top).put("bottom", top))
                    .put("mask", mask),
            ),
        )
        batch.put(
            JSONObject().put("op", "setClipTransform").put("clipId", id)
                .put("scale", cellW / (cutW * fit))
                .put("offsetX", cell.x + cell.w / 2 - 0.5)
                .put("offsetY", cell.y + cell.h / 2 - 0.5)
                .put("rotation", 0.0),
        )
    }
    batch.put(
        JSONObject().put("op", "setTimelineExtra").put("key", COLLAGE_KEY)
            .put(
                "value",
                JSONObject().put("ids", JSONArray(ids)).put("layout", layout).put("gap", c.gap.toDouble())
                    .put("radius", c.radius.toDouble()).put("color", c.color),
            ),
    )
    return apply(JSONObject().put("op", "batch").put("commands", batch).toString(), null)
}

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
    val kept = remember(vm.timeline?.revision) { vm.session?.collage() } ?: return
    var tab by remember { mutableStateOf(0) }
    var swapFrom by remember { mutableStateOf<Int?>(null) }
    val layouts = collageLayouts(kept.ids.size)
    fun lay(next: Collage) = vm.edit { layCollage(view(), next) }
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("Layout", "Borders", "Frame").forEachIndexed { i, name -> Pill(name, tab == i) { tab = i } }
        }
        when (tab) {
            0 -> Column(Modifier.fillMaxWidth().padding(top = 10.dp)) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Pictures", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(64.dp))
                    Pill("−", false) {
                        if (kept.ids.size > 2) {
                            val last = kept.ids.last()
                            vm.edit { delete(listOf(last)); layCollage(view(), kept.copy(ids = kept.ids.dropLast(1), layout = 0)) }
                        } else vm.message = "A collage keeps at least two pictures"
                    }
                    Text("${kept.ids.size}", style = Type.BodyStrong.copy(color = Palette.Text), modifier = Modifier.padding(horizontal = 12.dp))
                    Pill("+", false) {
                        if (kept.ids.size >= 9) vm.message = "A collage holds up to nine pictures"
                        else MediaPick.open(MediaKinds.BOTH, multiple = false) { uris ->
                            val uri = uris.firstOrNull() ?: return@open
                            val app = vm.getApplication<android.app.Application>()
                            vm.edit {
                                val media = import(Media.importUri(app, vm.path, uri).absolutePath)
                                val added = addOverlay(media.id, 0L)
                                layCollage(added, kept.copy(ids = kept.ids + added.createdId, layout = 0))
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
                if (swapFrom != null) Text("Tap another cell to swap", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.padding(horizontal = 12.dp))
                LazyRow(Modifier.fillMaxWidth().padding(top = 8.dp), contentPadding = PaddingValues(horizontal = 10.dp)) {
                    items(layouts.size) { i -> LayoutTile(layouts[i], i == kept.layout) { lay(kept.copy(layout = i)) } }
                }
            }
            1 -> Column(Modifier.fillMaxWidth().padding(top = 6.dp)) {
                ValueSlider(kept.gap * 1000f, 0f..60f, { "%.0f".format(it) }, { v -> lay(kept.copy(gap = v / 1000f)) }, { vm.endGesture() }, title = "Width")
                ValueSlider(kept.radius * 100f, 0f..100f, { "%.0f".format(it) }, { v -> lay(kept.copy(radius = v / 100f)) }, { vm.endGesture() }, title = "Radius")
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    GAP_COLOURS.forEach { hex ->
                        val on = hex.equals(kept.color, ignoreCase = true)
                        Box(
                            Modifier.size(30.dp).clip(CircleShape).background(if (on) Palette.Accent else Color.Transparent).padding(3.dp)
                                .clip(CircleShape).background(Color(android.graphics.Color.parseColor(hex))).clickable { lay(kept.copy(color = hex)) },
                        )
                    }
                }
            }
            else -> Row(Modifier.fillMaxWidth().padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val t = vm.timeline
                COLLAGE_FRAMES.forEach { (name, size) ->
                    val on = t != null && t.width.toInt() * size.second == t.height.toInt() * size.first
                    Pill(name, on) {
                        vm.edit {
                            setCanvas(size.first.toUInt(), size.second.toUInt())
                            layCollage(view(), kept)
                        }
                    }
                }
            }
        }
    }
}

/** A rounded label to tap, lit when [on]. */
@Composable
private fun Pill(label: String, on: Boolean, onClick: () -> Unit) {
    Box(Modifier.clip(RoundedCornerShape(16.dp)).background(if (on) Palette.Accent else Color(0xFF3A3A3C)).clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 7.dp)) {
        Text(label, style = Type.Small.copy(color = if (on) Palette.OnAccent else Palette.Text, fontSize = 13.sp))
    }
}

/** The current layout's cells, numbered; the one picked for a swap lit. */
@Composable
private fun CellMap(cells: List<Cell>, picked: Int?, onTap: (Int) -> Unit) {
    Canvas(
        Modifier.size(64.dp).pointerInput(cells) {
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
            drawRect(if (i == picked) Palette.Accent else Color(0xFF3A3A3C), at, box)
        }
    }
}

/** A layout drawn as its cells: outlined, and filled and cyan-edged when chosen. */
@Composable
private fun LayoutTile(cells: List<Cell>, selected: Boolean, onClick: () -> Unit) {
    Canvas(Modifier.padding(horizontal = 9.dp).size(48.dp).clickable(onClick = onClick)) {
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
