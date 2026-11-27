package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.core.SpeechListener
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.core.reelEffects
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.common.MediaKinds
import io.github.artcraftandroid.capcraft.ui.common.MediaPick
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** The setting a reel effect's own words go in. */
private const val TEXT = "text"

/** The clip's reel effects as the document holds them. */
private fun reelFx(vm: EditorViewModel, clipId: String): List<JSONObject> {
    val a = vm.clipJson(clipId)?.optJSONArray("reelFx") ?: return emptyList()
    return (0 until a.length()).map { a.getJSONObject(it) }
}

private fun setReelFx(vm: EditorViewModel, clipId: String, list: List<JSONObject>, gesture: String? = null) {
    vm.patch(clipId, JSONObject().put("reelFx", JSONArray(list)), gesture)
}

/** One setting an effect reads: its key, the hint shown in the empty box, and the looks to pick from, if any. */
private data class Opt(val key: String, val hint: String, val choices: List<String>)

private val WORD = Regex("[a-z_]+")

/**
 * The settings an effect reads, from its `key: what; key (default)` line. `images` is left out (the
 * picker sets it), as is `none`. A hint ending in `a, b or c` is offered as chips.
 */
private fun optsOf(line: String): List<Opt> = line.split(';').map { it.trim() }.filter { it.isNotEmpty() && it != "none" }.flatMap { part ->
    val cut = part.indexOfFirst { it == ':' || it == '(' }.let { if (it < 0) part.length else it }
    val head = part.substring(0, cut).trim()
    val hint = part.substring(cut).trimStart(':').trim().removePrefix("(").removeSuffix(")").trim().ifEmpty { head.substringAfter(' ', "") }
    val tail = hint.substringAfterLast(':').trim()
    val choices = if (" or " in tail) tail.split(",", " or ").map { it.trim() }.takeIf { c -> c.size >= 2 && c.all { WORD.matches(it) } }.orEmpty() else emptyList()
    val keys = if ("," in head) head.split(',').map { it.trim() } else listOf(head.substringBefore(' '))
    keys.filter { WORD.matches(it) && it != "images" }.map { Opt(it, hint, choices) }
}.distinctBy { it.key }

private fun labelOf(key: String): String = if (key == TEXT) "Words" else key.replace('_', ' ').replaceFirstChar { it.uppercase() }

/**
 * Reel effects: the 51 talking-head effects from claude-video-effects (MIT), drawn on the clip's own frames
 * around the person and the face. Added at the playhead for three seconds; each shows only the settings it reads.
 * The face and the person are found on the phone the first time, with the progress here.
 */
@Composable
fun ReelFxPanel(vm: EditorViewModel) {
    // a reel layer on the effect row, or a picture with effects of its own; nothing chosen adds one
    val selected = vm.selectedClip
    val clip = selected?.takeIf { it.kind == "video" || it.kind == "image" || (it.kind == "layer" && reelFx(vm, it.id).isNotEmpty()) }
    val layer = clip?.kind == "layer"
    val all = remember { runCatching { reelEffects() }.getOrDefault(emptyList()) }
    val list = remember(clip?.json) { clip?.let { reelFx(vm, it.id) }.orEmpty() }
    val key = clip?.id ?: ""
    var chosen by remember(key) { mutableStateOf<Int?>(null) }
    var adding by remember(key) { mutableStateOf(list.isEmpty()) }
    if (clip == null) adding = true
    var tracking by remember(key) { mutableStateOf<Float?>(null) }
    val scope = rememberCoroutineScope()
    val listener = remember(key) {
        object : SpeechListener {
            override fun onProgress(fraction: Float, stage: String) {
                scope.launch(Dispatchers.Main) { tracking = fraction }
            }
            override fun onDone(result: TimelineView?) {
                scope.launch(Dispatchers.Main) { tracking = null; result?.let { vm.applyView(it) } }
            }
            override fun onFailed(message: String) {
                scope.launch(Dispatchers.Main) { tracking = null; vm.message = message }
            }
        }
    }
    val any = list.isNotEmpty()
    LaunchedEffect(key, list.size) {
        val s = vm.session ?: return@LaunchedEffect
        if (!any || clip == null) return@LaunchedEffect
        withContext(Threads.commands) {
            runCatching {
                if (s.reelTrackingMissing(clip.id)) {
                    withContext(Dispatchers.Main) { tracking = 0f }
                    s.trackReel(clip.id, listener)
                }
            }.onFailure { e -> withContext(Dispatchers.Main) { tracking = null; vm.message = e.userMessage } }
        }
    }
    val lengthS = (clip?.durationUs ?: 0L) / 1_000_000.0
    PanelFrame("Reel FX", vm, height = 330.dp, titleOnTop = true) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
            tracking?.let { Text("Finding you  ${(it * 100).toInt()}%", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.padding(bottom = 6.dp)) }
            // the clip's effects, and Add; a layer is one effect, timed on the timeline
            if (layer) {
                Text("Move and trim it on the effects row. Its opacity, keyed, is how strong it is.", style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 11.sp))
                if (chosen == null) chosen = 0
            } else Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                list.forEachIndexed { i, e ->
                    val name = all.firstOrNull { it.name == e.optString("fx") }?.title ?: e.optString("fx")
                    Chip("$name  ${"%.1f".format(e.optDouble("start"))}–${"%.1f".format(e.optDouble("end"))}s", chosen == i && !adding) { chosen = i; adding = false }
                }
                Chip("+ Add", adding) { adding = true; chosen = null }
            }
            Spacer(Modifier.height(10.dp))
            if (adding) {
                LazyVerticalGrid(GridCells.Fixed(2), Modifier.fillMaxWidth().height(230.dp), verticalArrangement = Arrangement.spacedBy(6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(all) { fx ->
                        Column(
                            Modifier.clip(RoundedCornerShape(8.dp)).background(Color(0xFF2E2E30)).clickable {
                                // an item of its own on the effects row, selected so its settings show
                                vm.addReelLayer(fx.name, fx.title)
                                adding = false
                                chosen = 0
                            }.padding(10.dp),
                        ) {
                            Text(fx.title, style = Type.BodyStrong.copy(color = Palette.Text, fontSize = 14.sp), maxLines = 1)
                            Text(fx.desc, style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 11.sp), maxLines = 2, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
                return@Column
            }
            val c = clip ?: return@Column
            val i = chosen ?: return@Column
            val e = list.getOrNull(i) ?: return@Column
            val info = all.firstOrNull { it.name == e.optString("fx") }
            fun change(gesture: String? = null, edit: (JSONObject) -> Unit) {
                val copy = list.map { JSONObject(it.toString()) }
                edit(copy[i])
                setReelFx(vm, c.id, copy, gesture)
            }
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                info?.let { Text(it.desc, style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 11.sp), maxLines = 2, overflow = TextOverflow.Ellipsis) }
                if (!layer) {
                    ValueSlider(e.optDouble("start").toFloat(), 0f..lengthS.toFloat(), { "%.1fs".format(it) },
                        { v -> change("reelfx-start") { it.put("start", v.toDouble().coerceAtMost(it.optDouble("end") - 0.2)) } }, { vm.endGesture() }, title = "Start")
                    ValueSlider(e.optDouble("end").toFloat(), 0f..lengthS.toFloat(), { "%.1fs".format(it) },
                        { v -> change("reelfx-end") { it.put("end", v.toDouble().coerceAtLeast(it.optDouble("start") + 0.2)) } }, { vm.endGesture() }, title = "End")
                }
                val opts = e.optJSONObject("options")
                fun set(key: String, v: String) = change {
                    val o = it.optJSONObject("options") ?: JSONObject()
                    if (v.isBlank()) o.remove(key) else o.put(key, v)
                    it.put("options", o)
                }
                optsOf(info?.options.orEmpty()).forEach { o ->
                    if (o.choices.isNotEmpty()) {
                        val now = opts?.optString(o.key).orEmpty().ifEmpty { o.choices.first() }
                        Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(labelOf(o.key), style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(70.dp))
                            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                o.choices.forEach { c -> Chip(if (c == "bw") "B&W" else c.replaceFirstChar { it.uppercase() }, c == now) { set(o.key, c) } }
                            }
                        }
                    } else {
                        Field(labelOf(o.key), opts?.optString(o.key).orEmpty(), o.hint) { v -> set(o.key, v) }
                    }
                }
                Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (info?.cards == true) {
                        val context = androidx.compose.ui.platform.LocalContext.current
                        Chip("Your images", false) {
                            MediaPick.open(MediaKinds.IMAGE, multiple = true) { uris ->
                                scope.launch(Threads.commands) {
                                    // copies kept in the app's own storage, read by the engine
                                    val dir = java.io.File(context.filesDir, "reelfx").apply { mkdirs() }
                                    val paths = uris.mapNotNull { uri ->
                                        runCatching {
                                            val f = java.io.File(dir, "${uri.toString().hashCode().toUInt()}.img")
                                            context.contentResolver.openInputStream(uri)?.use { input -> f.outputStream().use { input.copyTo(it) } }
                                            f.absolutePath
                                        }.getOrNull()
                                    }
                                    withContext(Dispatchers.Main) {
                                        change { it.put("options", (it.optJSONObject("options") ?: JSONObject()).put("images", paths.joinToString("|"))) }
                                    }
                                }
                            }
                        }
                    }
                    Chip("Remove", false) {
                        if (layer) { vm.edit { delete(listOf(c.id)) }; vm.select(null); vm.panel = null } else setReelFx(vm, c.id, list.filterIndexed { j, _ -> j != i })
                        chosen = null
                    }
                }
            }
        }
    }
}

@Composable
private fun Chip(label: String, on: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.clip(RoundedCornerShape(16.dp)).background(if (on) Palette.Accent else Color(0xFF3A3A3C)).clickable(onClick = onClick).padding(horizontal = 12.dp, vertical = 7.dp),
    ) {
        Text(label, style = Type.Small.copy(color = if (on) Palette.OnAccent else Palette.Text, fontSize = 13.sp), maxLines = 1)
    }
}

/** A one-line text box that hands its words over when they change. */
@Composable
private fun Field(title: String, value: String, hint: String, onDone: (String) -> Unit) {
    var text by remember(value) { mutableStateOf(value) }
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(70.dp))
        BasicTextField(
            text,
            { text = it; onDone(it) },
            singleLine = true,
            textStyle = Type.Body.copy(color = Palette.Text, fontSize = 14.sp),
            cursorBrush = SolidColor(Palette.Accent),
            modifier = Modifier.weight(1f).clip(RoundedCornerShape(6.dp)).background(Color(0xFF2E2E30)).padding(horizontal = 10.dp, vertical = 8.dp),
            decorationBox = { inner ->
                Box {
                    if (text.isEmpty()) Text(hint, style = Type.Body.copy(color = Palette.TextSecondary, fontSize = 14.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    inner()
                }
            },
        )
    }
}
