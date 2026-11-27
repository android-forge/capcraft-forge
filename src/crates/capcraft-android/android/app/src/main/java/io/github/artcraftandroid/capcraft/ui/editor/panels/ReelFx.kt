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

/** The settings a reel effect's own words go in; the rest are typed as `key=value; key=value`. */
private const val TEXT = "text"

/** The clip's reel effects as the document holds them. */
private fun reelFx(vm: EditorViewModel, clipId: String): List<JSONObject> {
    val a = vm.clipJson(clipId)?.optJSONArray("reelFx") ?: return emptyList()
    return (0 until a.length()).map { a.getJSONObject(it) }
}

private fun setReelFx(vm: EditorViewModel, clipId: String, list: List<JSONObject>, gesture: String? = null) {
    vm.patch(clipId, JSONObject().put("reelFx", JSONArray(list)), gesture)
}

/** `key=value; key=value` of an effect's settings, its words left out. */
private fun optionsLine(o: JSONObject?): String =
    o?.keys()?.asSequence()?.filter { it != TEXT && it != "images" }?.joinToString("; ") { "$it=${o.optString(it)}" }.orEmpty()

/**
 * Reel effects: the 51 talking-head effects from claude-video-effects (MIT), drawn on the clip's own frames
 * around the person and the face. Added at the playhead for three seconds; each can be timed, worded and set.
 * The face and the person are found on the phone the first time, with the progress here.
 */
@Composable
fun ReelFxPanel(vm: EditorViewModel) {
    val clip = vm.selectedClip?.takeIf { it.kind == "video" || it.kind == "image" } ?: run { vm.panel = null; return }
    val all = remember { runCatching { reelEffects() }.getOrDefault(emptyList()) }
    val list = remember(clip.json) { reelFx(vm, clip.id) }
    var chosen by remember(clip.id) { mutableStateOf<Int?>(null) }
    var adding by remember(clip.id) { mutableStateOf(list.isEmpty()) }
    var tracking by remember(clip.id) { mutableStateOf<Float?>(null) }
    val scope = rememberCoroutineScope()
    val listener = remember(clip.id) {
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
    LaunchedEffect(clip.id, list.size) {
        val s = vm.session ?: return@LaunchedEffect
        if (!any) return@LaunchedEffect
        withContext(Threads.commands) {
            runCatching {
                if (s.reelTrackingMissing(clip.id)) {
                    withContext(Dispatchers.Main) { tracking = 0f }
                    s.trackReel(clip.id, listener)
                }
            }.onFailure { e -> withContext(Dispatchers.Main) { tracking = null; vm.message = e.userMessage } }
        }
    }
    val lengthS = clip.durationUs / 1_000_000.0
    PanelFrame("Reel FX", vm, height = 330.dp, titleOnTop = true) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
            tracking?.let { Text("Finding you  ${(it * 100).toInt()}%", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.padding(bottom = 6.dp)) }
            // the clip's effects, and Add
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
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
                                val at = ((vm.playhead - clip.startUs) / 1_000_000.0).coerceIn(0.0, (lengthS - 0.5).coerceAtLeast(0.0))
                                val e = JSONObject().put("fx", fx.name).put("start", at).put("end", (at + 3.0).coerceAtMost(lengthS)).put("options", JSONObject())
                                setReelFx(vm, clip.id, list + e)
                                chosen = list.size
                                adding = false
                            }.padding(10.dp),
                        ) {
                            Text(fx.title, style = Type.BodyStrong.copy(color = Palette.Text, fontSize = 14.sp), maxLines = 1)
                            Text(fx.desc, style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 11.sp), maxLines = 2, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
                return@Column
            }
            val i = chosen ?: return@Column
            val e = list.getOrNull(i) ?: return@Column
            val info = all.firstOrNull { it.name == e.optString("fx") }
            fun change(gesture: String? = null, edit: (JSONObject) -> Unit) {
                val copy = list.map { JSONObject(it.toString()) }
                edit(copy[i])
                setReelFx(vm, clip.id, copy, gesture)
            }
            Column(Modifier.fillMaxWidth()) {
                info?.let { Text(it.options, style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 11.sp), maxLines = 2, overflow = TextOverflow.Ellipsis) }
                ValueSlider(e.optDouble("start").toFloat(), 0f..lengthS.toFloat(), { "%.1fs".format(it) },
                    { v -> change("reelfx-start") { it.put("start", v.toDouble().coerceAtMost(it.optDouble("end") - 0.2)) } }, { vm.endGesture() }, title = "Start")
                ValueSlider(e.optDouble("end").toFloat(), 0f..lengthS.toFloat(), { "%.1fs".format(it) },
                    { v -> change("reelfx-end") { it.put("end", v.toDouble().coerceAtLeast(it.optDouble("start") + 0.2)) } }, { vm.endGesture() }, title = "End")
                val opts = e.optJSONObject("options")
                Field("Words", opts?.optString(TEXT).orEmpty()) { v -> change { it.put("options", (it.optJSONObject("options") ?: JSONObject()).put(TEXT, v)) } }
                Field("Settings", optionsLine(opts)) { v ->
                    change {
                        val o = JSONObject()
                        (it.optJSONObject("options") ?: JSONObject()).let { old ->
                            listOf(TEXT, "images").forEach { k -> if (old.has(k)) o.put(k, old.get(k)) }
                        }
                        v.split(';').map { it.trim() }.filter { "=" in it }.forEach { kv -> o.put(kv.substringBefore('=').trim(), kv.substringAfter('=').trim()) }
                        it.put("options", o)
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
                        setReelFx(vm, clip.id, list.filterIndexed { j, _ -> j != i })
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
private fun Field(title: String, value: String, onDone: (String) -> Unit) {
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
        )
    }
}
