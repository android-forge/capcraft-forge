package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.util.LruCache
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.core.drawGraphic
import io.github.artcraftandroid.capcraft.engine.intoBitmap
import io.github.artcraftandroid.capcraft.ui.Glyph
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import io.github.artcraftandroid.capcraft.ui.editor.Panel
import java.io.File
import java.io.FileOutputStream
import java.util.zip.Inflater
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** One set of graphics: whose it is, and whether it takes a colour ([mono]) or asks for a credit where it is shown. */
internal class GraphicSet(val id: String, val title: String, val licence: String, val author: String, val link: String, val credit: Boolean, val mono: Boolean)

/** One graphic of the library: where its picture is, and the words it is found by. */
internal class Graphic(val set: GraphicSet, val name: String, val words: String, val width: Float, val height: Float, val block: Int, val at: Int, val length: Int) {
    val key: String get() = set.id + "/" + name
    val label: String get() = name.replace('-', ' ')
}

/** The graphics library in the app's assets (graphics/build.py packs it): read once, its pictures unpacked as they are shown. */
internal object GraphicLibrary {
    private class Block(val at: Int, val packed: Int, val size: Int)

    private var blocks: List<Block> = emptyList()

    @Volatile var sets: List<GraphicSet> = emptyList()
        private set

    @Volatile var all: List<Graphic> = emptyList()
        private set
    private var byKey: Map<String, Graphic> = emptyMap()
    private val unpacked = LruCache<Int, ByteArray>(14)
    private val small = LruCache<String, Bitmap>(400)

    private fun inflate(packed: ByteArray, size: Int?): ByteArray {
        val inflater = Inflater(true)
        try {
            inflater.setInput(packed)
            if (size != null) return ByteArray(size).also { out -> var n = 0; while (n < size) { val k = inflater.inflate(out, n, size - n); if (k <= 0) break; n += k } }
            val out = java.io.ByteArrayOutputStream(packed.size * 5)
            val piece = ByteArray(64 * 1024)
            while (!inflater.finished()) {
                val k = inflater.inflate(piece)
                if (k <= 0) break
                out.write(piece, 0, k)
            }
            return out.toByteArray()
        } finally {
            inflater.end()
        }
    }

    /** Reads what there is, the first time it is asked. */
    @Synchronized
    fun load(context: Context): List<Graphic> {
        if (all.isNotEmpty()) return all
        val lines = String(inflate(context.assets.open("graphics/index.bin").use { it.readBytes() }, null)).lineSequence()
        val found = ArrayList<Graphic>(21_000)
        val kinds = ArrayList<GraphicSet>()
        val where = ArrayList<Block>()
        for (line in lines) {
            if (line.isEmpty()) continue
            val f = line.split('\t')
            when {
                f[0] == "@" -> where.add(Block(f[1].toInt(), f[2].toInt(), f[3].toInt()))
                f[0] == "#" -> kinds.add(GraphicSet(f[1], f[2], f[3], f[4], f[5], f[6] == "1", f[7] == "1"))
                else -> found.add(Graphic(kinds.last(), f[0], f[1], f[2].toFloat(), f[3].toFloat(), f[4].toInt(), f[5].toInt(), f[6].toInt()))
            }
        }
        blocks = where
        sets = kinds
        byKey = found.associateBy { it.key }
        all = found
        return found
    }

    fun of(key: String): Graphic? = byKey[key]

    private fun body(context: Context, g: Graphic): String {
        val bytes = synchronized(unpacked) { unpacked.get(g.block) } ?: run {
            val b = blocks[g.block]
            // stored as it is, so it can be read from the middle
            val packed = context.assets.open("graphics/shapes.bin").use { s ->
                var skip = b.at.toLong()
                while (skip > 0) { val k = s.skip(skip); if (k <= 0) break; skip -= k }
                ByteArray(b.packed).also { java.io.DataInputStream(s).readFully(it) }
            }
            inflate(packed, b.size).also { synchronized(unpacked) { unpacked.put(g.block, it) } }
        }
        return String(bytes, g.at, g.length, Charsets.UTF_8)
    }

    fun draw(context: Context, g: Graphic, side: Int, color: String): Bitmap? =
        runCatching { drawGraphic(body(context, g), g.width, g.height, side.toUInt(), color).intoBitmap(null) }.getOrNull()

    fun tile(context: Context, g: Graphic, color: String): Bitmap? {
        val key = g.key + if (g.set.mono) color else ""
        small.get(key)?.let { return it }
        return draw(context, g, 132, color)?.also { small.put(key, it) }
    }

    /** A tile already drawn, so that one coming back on the screen does not blink. */
    fun drawn(g: Graphic, color: String): Bitmap? = small.get(g.key + if (g.set.mono) color else "")

    /**
     * What [query] finds: every word of it must start one of a graphic's. Those named so come first, then those
     * known as it; the words of a name count for more the fewer they are.
     */
    fun search(query: String, from: List<Graphic>, limit: Int = 400): List<Graphic> {
        val asked = query.lowercase().split(Regex("[^a-z0-9]+")).filter { it.isNotEmpty() }
        if (asked.isEmpty()) return from
        val hits = ArrayList<Pair<Int, Graphic>>()
        for (g in from) {
            val name = g.name.split('-')
            var score = 0
            for (word in asked) {
                // "cars" is looked for as "car" too
                val stem = if (word.length > 3 && word.endsWith("s")) word.dropLast(1) else word
                val s = when {
                    name.any { it == word || it == stem } -> 100
                    name.any { it.startsWith(stem) } -> 60
                    starts(g.words, stem, exact = true) -> 40
                    starts(g.words, stem, exact = false) -> 18
                    else -> 0
                }
                if (s == 0) { score = 0; break }
                score += s
            }
            if (score > 0) hits.add(score * 10 - name.size to g)
        }
        // a stable sort: those that match alike stay in the shelves' order
        return hits.sortedByDescending { it.first }.take(limit).map { it.second }
    }

    private fun starts(words: String, with: String, exact: Boolean): Boolean {
        var i = words.indexOf(with)
        while (i >= 0) {
            val end = i + with.length
            if ((i == 0 || words[i - 1] == ' ') && (!exact || end == words.length || words[end] == ' ')) return true
            i = words.indexOf(with, i + 1)
        }
        return false
    }
}

private val INKS = listOf("#ffffff", "#111111", "#ff3b30", "#ff9500", "#ffd60a", "#34c759", "#00c7be", "#2979ff", "#7c4dff", "#ff4081", "#8d6e63", "#9e9e9e")

/** A graphic as a transparent PNG in the project's media folder, drawn once for each colour. */
private fun graphicFile(context: Context, projectPath: String, g: Graphic, color: String): File? = runCatching {
    val dir = File(projectPath, "media").apply { mkdirs() }
    val ink = if (g.set.mono) "-" + color.drop(1) else ""
    val file = File(dir, "sticker-g-${g.set.id}-${g.name.take(60)}$ink.png")
    if (!file.exists()) {
        val b = GraphicLibrary.draw(context, g, 1024, color) ?: return null
        val part = File(dir, file.name + ".part")
        FileOutputStream(part).use { b.compress(Bitmap.CompressFormat.PNG, 100, it) }
        part.renameTo(file)
    }
    file
}.getOrNull()

/**
 * Graphics: twenty thousand vector pictures kept in the app, found by words without the internet. One is
 * drawn sharp, placed at the playhead like a sticker, and its time on screen set right there.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun GraphicsPanel(vm: EditorViewModel, first: String) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var query by remember { mutableStateOf(first) }
    var all by remember { mutableStateOf(GraphicLibrary.all) }
    // -1 is what was kept
    var shelf by remember { mutableIntStateOf(0) }
    var ink by remember { mutableStateOf(INKS.first()) }
    var placed by remember { mutableStateOf<Graphic?>(null) }
    val kept = remember(Favorites.version.intValue) { Favorites.of(context, "graphic") }
    LaunchedEffect(Unit) { if (all.isEmpty()) all = withContext(Dispatchers.Default) { GraphicLibrary.load(context) } }
    val sets = remember(all) { GraphicLibrary.sets }
    var shown by remember { mutableStateOf<List<Graphic>>(emptyList()) }
    LaunchedEffect(all, query, shelf, kept) {
        if (query.isNotBlank()) delay(120)
        shown = withContext(Dispatchers.Default) {
            when {
                query.isNotBlank() -> GraphicLibrary.search(query, all)
                shelf < 0 -> kept.mapNotNull { GraphicLibrary.of(it) }.sortedBy { it.name }
                else -> sets.getOrNull(shelf)?.let { set -> all.filter { it.set === set } }.orEmpty()
            }
        }
    }
    fun add(g: Graphic) {
        scope.launch {
            val file = withContext(Dispatchers.IO) { graphicFile(context, vm.path, g, ink) }
            if (file == null) vm.message = "That graphic could not be drawn" else {
                vm.addSticker(file)
                placed = g
            }
        }
    }

    PanelFrame("Graphics", vm, tall = true, titleOnTop = true, header = { SearchField(query, { query = it }, "Search ${if (all.isEmpty()) "" else "%,d ".format(all.size)}graphics") }) {
        Row(
            Modifier.fillMaxWidth().height(44.dp).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier.size(34.dp).clip(CircleShape).background(if (query.isBlank() && shelf == -1) Palette.Accent else Color(0xFF3A3A3A)).clickable { shelf = -1; query = "" },
                contentAlignment = Alignment.Center,
            ) { Glyph(LucideIcons.Bookmark, if (query.isBlank() && shelf == -1) Palette.OnAccent else Palette.DockText, size = 17.dp, description = "Kept") }
            sets.forEachIndexed { i, set -> Pill(set.title, query.isBlank() && shelf == i) { shelf = i; query = "" } }
        }
        // what takes one colour takes it from here
        if (shown.any { it.set.mono }) {
            Row(
                Modifier.fillMaxWidth().height(38.dp).horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                INKS.forEach { c ->
                    Box(
                        Modifier.size(26.dp).clip(CircleShape).background(Color(android.graphics.Color.parseColor(c)))
                            .border(if (c == ink) 3.dp else 1.dp, if (c == ink) Palette.Accent else Color(0x66FFFFFF), CircleShape).clickable { ink = c },
                    )
                }
            }
        }
        val clip = vm.selectedClip?.takeIf { placed != null && it.kind == "image" }
        if (clip != null) {
            WhenRow(vm)
            placed?.set?.takeIf { it.credit }?.let { set ->
                Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Asks for a credit: ${set.author}, ${set.licence}", style = Type.Tiny, modifier = Modifier.weight(1f))
                    Text(
                        "Copy credit",
                        style = Type.Small.copy(color = Palette.Accent),
                        modifier = Modifier.clickable {
                            val text = "Graphics: ${set.author} (${set.link}), CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/)"
                            (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Credit", text))
                            vm.message = "Credit copied"
                        }.padding(8.dp),
                    )
                }
            }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.Timeline))
        if (shown.isEmpty() && all.isNotEmpty()) {
            Text(if (shelf == -1 && query.isBlank()) "Press and hold one to keep it here" else "No graphics found", style = Type.Small, modifier = Modifier.padding(24.dp))
        }
        // a new search or shelf starts at its top
        val grid = androidx.compose.foundation.lazy.grid.rememberLazyGridState()
        LaunchedEffect(shown) { runCatching { grid.scrollToItem(0) } }
        LazyVerticalGrid(
            GridCells.Fixed(5),
            Modifier.fillMaxWidth().weight(1f),
            state = grid,
            contentPadding = PaddingValues(12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(shown, key = { it.key }) { g ->
                val picture by produceState(GraphicLibrary.drawn(g, ink), g.key, ink) { value = GraphicLibrary.drawn(g, ink) ?: withContext(Dispatchers.Default) { GraphicLibrary.tile(context, g, ink) } }
                Box(
                    // dark ink shows on a light tile
                    Modifier.aspectRatio(1f).clip(RoundedCornerShape(8.dp)).background(if (g.set.mono && ink == "#111111" || g.set.id == "streamline-freehand-color") Color(0xFFBDBDBD) else Color(0xFF333333))
                        .combinedClickable(
                            onClick = { add(g) },
                            onLongClick = { vm.message = if (Favorites.toggle(context, "graphic", g.key)) "Kept" else "No longer kept" },
                        ),
                    contentAlignment = Alignment.Center,
                ) {
                    picture?.let { Image(it.asImageBitmap(), contentDescription = g.label, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize().padding(9.dp)) }
                    if (g.key in kept) Box(Modifier.align(Alignment.TopEnd).padding(3.dp).size(15.dp).clip(CircleShape).background(Color(0x99000000)), contentAlignment = Alignment.Center) { Glyph(LucideIcons.Bookmark, Color.White, size = 9.dp) }
                }
            }
        }
    }
}

@Composable
private fun Pill(label: String, on: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.height(34.dp).clip(RoundedCornerShape(17.dp)).background(if (on) Palette.Accent else Color(0xFF3A3A3A)).clickable(onClick = onClick).padding(horizontal = 13.dp),
        contentAlignment = Alignment.Center,
    ) { Text(label, style = Type.Small.copy(fontSize = 13.sp, color = if (on) Palette.OnAccent else Palette.Text)) }
}

private fun tenths(us: Long): String = "%d:%04.1f".format(us / 60_000_000, us % 60_000_000 / 1e6)

/** When the selected clip shows, in a line: from where, for how long, and the ways to set both at once. */
@Composable
private fun WhenRow(vm: EditorViewModel) {
    val clip = vm.selectedClip ?: return
    Row(
        Modifier.fillMaxWidth().height(40.dp).horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(LucideIcons.Timer, Palette.TextSecondary, size = 16.dp)
        Text("${tenths(clip.startUs)} – ${tenths(clip.startUs + clip.durationUs)}", style = Type.Small.copy(color = Palette.Text))
        listOf(1, 2, 3, 5, 10).forEach { s ->
            Pill("${s}s", clip.durationUs == s * 1_000_000L) { vm.edit(select = { clip.id }) { setWindow(clip.id, clip.startUs, s * 1_000_000L) } }
        }
        Pill("Whole video", false) { wholeVideo(vm) }
        Pill("More", false) { vm.panel = Panel.Timing }
    }
}

private fun wholeVideo(vm: EditorViewModel) {
    val clip = vm.selectedClip ?: return
    val end = mainEnd(vm)
    if (end > 0) vm.edit(select = { clip.id }) { setWindow(clip.id, 0L, end) }
}

/** Where the main track ends: what a clip over it can last to. */
private fun mainEnd(vm: EditorViewModel): Long =
    vm.timeline?.rows?.firstOrNull { it.kind == io.github.artcraftandroid.capcraft.core.RowKind.MAIN }?.clips?.maxOfOrNull { it.startUs + it.durationUs } ?: (vm.timeline?.durationUs ?: 0L)

/** Timing: when a graphic, a title or an overlay comes on screen and goes, to a tenth of a second. */
@Composable
fun TimingPanel(vm: EditorViewModel) {
    val clip = vm.selectedClip ?: run { vm.panel = null; return }
    val end = maxOf(mainEnd(vm), clip.startUs + clip.durationUs, 1_000_000L)
    var start by remember(clip.id, clip.startUs, clip.durationUs) { mutableFloatStateOf(clip.startUs / 1e6f) }
    var stop by remember(clip.id, clip.startUs, clip.durationUs) { mutableFloatStateOf((clip.startUs + clip.durationUs) / 1e6f) }
    fun set(from: Float, to: Float) {
        val a = (from * 10).roundToInt() * 100_000L
        val b = maxOf((to * 10).roundToInt() * 100_000L, a + 100_000L)
        vm.edit(select = { clip.id }) { setWindow(clip.id, a, b - a) }
    }
    PanelFrame("Timing", vm, height = 250.dp, left = {
        Text("Animations", style = Type.Small.copy(color = Palette.DockText), modifier = Modifier.clickable { vm.panel = Panel.Animation }.padding(8.dp))
    }) {
        Column(Modifier.fillMaxWidth().weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically)) {
            Text(
                "Shows for ${"%.1f".format((stop - start).coerceAtLeast(0f))} s",
                style = Type.Small, modifier = Modifier.align(Alignment.CenterHorizontally),
            )
            ValueSlider(
                value = start, range = 0f..(end / 1e6f), label = { tenths((it * 1e6f).toLong()) },
                onChange = { start = it.coerceAtMost(stop - 0.1f).coerceAtLeast(0f) }, onDone = { set(start, stop) }, title = "Comes on",
            )
            ValueSlider(
                value = stop, range = 0f..(end / 1e6f), label = { tenths((it * 1e6f).toLong()) },
                onChange = { stop = it.coerceAtLeast(start + 0.1f) }, onDone = { set(start, stop) }, title = "Goes",
            )
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 14.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Pill("Comes on here", false) { val t = vm.playhead / 1e6f; set(t, maxOf(stop, t + 0.5f)) }
                Pill("Goes here", false) { val t = vm.playhead / 1e6f; if (t > start + 0.1f) set(start, t) else vm.message = "Move the playhead past where it comes on" }
                Pill("Whole video", false) { wholeVideo(vm) }
                Pill("To the end", false) { set(start, mainEnd(vm) / 1e6f) }
            }
            Spacer(Modifier.height(4.dp))
        }
    }
}
