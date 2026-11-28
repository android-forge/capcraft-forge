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
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import io.github.artcraftandroid.capcraft.ui.LucideIcons
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
import io.github.artcraftandroid.capcraft.core.reelFonts
import io.github.artcraftandroid.capcraft.core.reelSettings
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.common.MediaKinds
import io.github.artcraftandroid.capcraft.ui.common.MediaPick
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import io.github.artcraftandroid.capcraft.ui.editor.Panel
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

private val LABELS = mapOf(
    TEXT to "Words", "hl" to "Highlight", "tc" to "Corner tag", "sub" to "Subtitle", "label_x" to "Label at", "bg" to "Background",
    "ticks" to "Times (s)", "others" to "Other items", "pick" to "Picture", "cell" to "Dot size", "y" to "Height", "x" to "Across",
    "at" to "At (s)", "to" to "Values", "from" to "From", "push" to "Push in", "dark" to "Darken", "pos" to "Place", "count" to "Count",
)

private fun labelOf(key: String, hint: String = ""): String {
    // "the magazine name" names it better than the key does
    val named = hint.takeIf { it.startsWith("the ") }?.removePrefix("the ")?.substringBefore(" (")?.substringBefore(",")?.substringBefore(":")
        ?.takeIf { it.split(' ').size <= 3 }?.replaceFirstChar { it.uppercase() }
    return when {
        named != null && key in setOf("color", "bg", "ink") -> "$named colour"
        named != null -> named
        else -> LABELS[key] ?: key.replace('_', ' ').replaceFirstChar { it.uppercase() }
    }
}

/** Settings read as words that hold a number. */
private val NUMBER_KEYS = setOf("x", "y", "at", "pick", "opacity")

/** Numbers that count things. */
private val WHOLE_KEYS = setOf("count", "from", "pick", "spin", "angle", "box", "pages", "spread")

/** More looks for an effect than its line names. */
private val EXTRA_LOOKS = mapOf<String, List<String>>()

/** The slider's span for a number setting with default [d]. */
private fun rangeOf(key: String, d: Float, hint: String, lengthS: Float): ClosedFloatingPointRange<Float> = when {
    key == "zoom" || key == "push" -> 1f..maxOf(2.5f, d * 1.5f)
    key == "angle" -> -45f..45f
    key == "spin" -> 0f..40f
    key == "from" -> 1f..10f
    key == "pick" -> 0f..9f
    key == "box" -> 1f..6f
    key == "pages" -> 3f..16f
    key == "spread" -> 20f..180f
    key == "tilt" -> -1f..1f
    key == "count" && d >= 1000f -> 0f..maxOf(1_000_000f, d * 5f)
    key == "count" -> 1f..maxOf(40f, d * 3f)
    key == "at" && "0 to 1" !in hint -> 0f..lengthS
    key == "speed" -> 0f..maxOf(2f, d * 3f)
    "0 to 1" in hint || d <= 1f -> 0f..1f
    else -> 0f..d * 3f
}

/** Sounds that suit an effect, offered first. */
private val SUITS = mapOf(
    "profile-wall" to listOf("Sparkle", "Coin", "Success"),
    "slow-push" to listOf("Riser", "Drone", "Whoosh"),
    "countdown-flip" to listOf("Tick tock", "Ding"),
    "the-tap" to listOf("Pop", "Notify"),
    "tap" to listOf("Pop", "Notify"),
    "uninstall" to listOf("Glitch", "Error"),
    "reject-stamp" to listOf("Thud", "Impact"),
    "huge-strike" to listOf("Impact", "Boom"),
    "messages" to listOf("Notify", "Phone buzz"),
    "shortcut-rain" to listOf("Keyboard", "Typewriter"),
)

/** What a screen reader calls a colour dot. */
internal fun colourName(hex: String): String = when (hex.lowercase()) {
    "#ffffff" -> "White"
    "#000000" -> "Black"
    "#f4e9d8" -> "Cream"
    "#ffd60a" -> "Yellow"
    "#ff9230" -> "Orange"
    "#ee2222" -> "Red"
    "#ff4fa3" -> "Pink"
    "#7c5cff" -> "Purple"
    "#0a84ff" -> "Blue"
    "#34c759", "#22c55e" -> "Green"
    "#141416", "#2e2e30" -> "Charcoal"
    "#9ad7c4" -> "Mint"
    else -> "Colour $hex"
}

/** Colours to pick from. */
internal val PICK_COLOURS = listOf(
    "#ffffff", "#f4e9d8", "#ffd60a", "#ff9230", "#ee2222", "#ff4fa3", "#7c5cff", "#0a84ff", "#34c759", "#9ad7c4", "#2e2e30", "#000000",
)

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
                Text("Move and trim it on the effects row. Its opacity, keyed, is how strong it is.", style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 12.sp))
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
                            Modifier.clip(RoundedCornerShape(8.dp)).background(Palette.CardPressed).clickable {
                                // an item of its own on the effects row, selected so its settings show
                                vm.addReelLayer(fx.name, fx.title)
                                adding = false
                                chosen = 0
                            }.padding(10.dp),
                        ) {
                            Text(fx.title, style = Type.BodyStrong.copy(color = Palette.Text, fontSize = 14.sp), maxLines = 1)
                            Text(fx.desc, style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 12.sp), maxLines = 2, overflow = TextOverflow.Ellipsis)
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
                info?.let { Text(it.desc, style = Type.Small.copy(color = Palette.TextSecondary, fontSize = 12.sp), maxLines = 2, overflow = TextOverflow.Ellipsis) }
                if (!layer) {
                    ValueSlider(e.optDouble("start").toFloat(), 0f..lengthS.toFloat(), { "%.1fs".format(it) },
                        { v -> change("reelfx-start") { it.put("start", v.toDouble().coerceAtMost(it.optDouble("end") - 0.2)) } }, { vm.endGesture() }, title = "Start")
                    ValueSlider(e.optDouble("end").toFloat(), 0f..lengthS.toFloat(), { "%.1fs".format(it) },
                        { v -> change("reelfx-end") { it.put("end", v.toDouble().coerceAtLeast(it.optDouble("start") + 0.2)) } }, { vm.endGesture() }, title = "End")
                }
                val opts = e.optJSONObject("options")
                // typing is one undo step per field, ended when the field is left; a tap is a step of its own
                fun set(key: String, v: String, gesture: String? = null) = change(gesture) {
                    val o = it.optJSONObject("options") ?: JSONObject()
                    if (v.isBlank()) o.remove(key) else o.put(key, v)
                    it.put("options", o)
                }
                val name = e.optString("fx")
                val read = remember(name) { runCatching { reelSettings(name) }.getOrDefault(emptyList()) }
                val hints = remember(name) { optsOf(info?.options.orEmpty()) }
                val fonts = remember { runCatching { reelFonts() }.getOrDefault(emptyList()) }
                // what the effect reads, then what its line names that this run did not reach
                val keys = (read.map { it.key } + hints.map { it.key }).distinct().filter { it != "images" }
                keys.forEach { key ->
                    val got = read.firstOrNull { it.key == key }
                    val hint = hints.firstOrNull { it.key == key }
                    val default = got?.defaultValue.orEmpty()
                    val now = opts?.optString(key).orEmpty()
                    val label = labelOf(key, hint?.hint.orEmpty())
                    val numeric = got?.kind == "number" || key in NUMBER_KEYS
                    when {
                        key == "font" -> Choices(label, listOf("bold", "serif") + fonts, now.ifEmpty { default.ifEmpty { "bold" } }, { it.replaceFirstChar { c -> c.uppercase() } }) { set(key, it) }
                        key == "hl" -> {
                            val words = (opts?.optString(TEXT).orEmpty().ifEmpty { read.firstOrNull { it.key == TEXT }?.defaultValue.orEmpty() })
                                .split('|', ' ').map { it.trim(' ', '.', ',', '!', '?') }.filter { it.isNotEmpty() }.distinct()
                            Choices(label, words, now.ifEmpty { default }, { it }) { set(key, it) }
                        }
                        hint != null && hint.choices.isNotEmpty() -> Choices(label, hint.choices, now.ifEmpty { default.ifEmpty { hint.choices.first() } }, { if (it == "bw") "B&W" else it.replaceFirstChar { c -> c.uppercase() } }) { set(key, it) }
                        // a named look: as made, or the one its line names
                        key == "look" || key == "pos" -> {
                            val other = hint?.hint?.substringBefore(" for ")?.trim()?.takeIf { WORD.matches(it) }
                            val looks = listOfNotNull("", other) + (EXTRA_LOOKS[name] ?: emptyList())
                            Choices(label, looks.distinct(), now, { if (it.isEmpty()) "Standard" else it.replaceFirstChar { c -> c.uppercase() } }) { set(key, it) }
                        }
                        got?.kind == "color" || key == "color" || key == "bg" -> Swatches(label, now.ifEmpty { default.ifEmpty { "#ffffff" } }) { set(key, it) }
                        numeric -> {
                            val d = default.toFloatOrNull() ?: 0.5f
                            val range = rangeOf(key, d, hint?.hint.orEmpty(), lengthS.toFloat().coerceAtLeast(1f))
                            val whole = key in WHOLE_KEYS
                            ValueSlider((now.toFloatOrNull() ?: d).coerceIn(range), range, { if (whole) "%.0f".format(it) else "%.2f".format(it) },
                                { v -> change("reelfx-$key") { val o = it.optJSONObject("options") ?: JSONObject(); o.put(key, if (whole) "%.0f".format(v) else "%.3f".format(v)); it.put("options", o) } },
                                { vm.endGesture() }, title = label)
                        }
                        '|' in default || hint?.hint?.contains("joined with |") == true -> Lines(label, now.ifEmpty { default }, { vm.endGesture() }) { set(key, it, "reelfx-$key") }
                        else -> Field(label, now.ifEmpty { default }, hint?.hint.orEmpty(), { vm.endGesture() }) { v -> set(key, v, "reelfx-$key") }
                    }
                }
                // a sound at the effect's start, the ones that suit it first
                val sounds = (SUITS[name].orEmpty() + listOf("Whoosh", "Pop", "Riser", "Impact", "Sparkle", "Ding", "Camera flash", "Typewriter", "Glitch")).distinct()
                Choices("Sound", sounds, "", { it }, choice = false) { sound ->
                    vm.scrub(c.startUs + (if (layer) 0L else (e.optDouble("start") * 1_000_000).toLong()))
                    addSoundFx(vm, sound, scope)
                }
                Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    if (info?.cards == true || read.any { it.key == "images" }) {
                        val context = androidx.compose.ui.platform.LocalContext.current
                        Chip("Your images", false, choice = false) {
                            MediaPick.open(MediaKinds.IMAGE, multiple = true) { uris ->
                                scope.launch(Threads.commands) {
                                    // copies kept in the project's own media, as any import is: they go with the project
                                    val paths = uris.mapNotNull { uri -> runCatching { io.github.artcraftandroid.capcraft.engine.Media.importUri(context, vm.path, uri).absolutePath }.getOrNull() }
                                    withContext(Dispatchers.Main) {
                                        change { it.put("options", (it.optJSONObject("options") ?: JSONObject()).put("images", paths.joinToString("|"))) }
                                    }
                                }
                            }
                        }
                    }
                    Chip("Remove", false, choice = false) {
                        if (layer) { vm.edit { delete(listOf(c.id)) }; vm.select(null); vm.panel = null } else setReelFx(vm, c.id, list.filterIndexed { j, _ -> j != i })
                        chosen = null
                    }
                }
            }
        }
    }
}

/** A row of chips, one chosen. */
@Composable
private fun Choices(title: String, choices: List<String>, now: String, name: (String) -> String, choice: Boolean = true, onPick: (String) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(70.dp))
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            choices.forEach { c -> Chip(name(c), c.equals(now, ignoreCase = true), choice) { onPick(c) } }
        }
    }
}

/** Colours as dots to tap; the one in use ringed. */
@Composable
private fun Swatches(title: String, now: String, onPick: (String) -> Unit) {
    val all = (listOf(now.lowercase()) + PICK_COLOURS).distinct()
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(70.dp))
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            all.forEach { hex ->
                val color = runCatching { Color(android.graphics.Color.parseColor(hex)) }.getOrDefault(Color.White)
                val on = hex.equals(now, ignoreCase = true)
                Box(
                    Modifier.minimumInteractiveComponentSize()
                        .semantics { contentDescription = colourName(hex) }
                        .selectable(selected = on, role = Role.RadioButton) { onPick(hex) },
                    contentAlignment = Alignment.Center,
                ) {
                    Box(
                        Modifier.size(30.dp).clip(androidx.compose.foundation.shape.CircleShape)
                            .background(if (on) Palette.Accent else Color.Transparent).padding(3.dp)
                            .clip(androidx.compose.foundation.shape.CircleShape).background(color),
                    )
                }
            }
        }
    }
}

/** Several lines, each its own box, with add and remove; held joined by `|`. Lines with `::` get a small second box. */
@Composable
private fun Lines(title: String, value: String, onLeave: () -> Unit, onDone: (String) -> Unit) {
    val items = value.split('|')
    Column(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        Text(title, style = Type.Small.copy(color = Palette.TextSecondary))
        items.forEachIndexed { i, item ->
            fun put(v: String) = onDone(items.toMutableList().also { it[i] = v }.joinToString("|"))
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                if ("::" in item) {
                    Box(Modifier.weight(0.62f)) { Box_(item.substringBefore("::"), onLeave) { put(it + "::" + item.substringAfter("::")) } }
                    Spacer(Modifier.width(6.dp))
                    Box(Modifier.weight(0.38f)) { Box_(item.substringAfter("::"), onLeave) { put(item.substringBefore("::") + "::" + it) } }
                } else {
                    Box(Modifier.weight(1f)) { Box_(item, onLeave) { put(it) } }
                }
                if (items.size > 1) {
                    Box(
                        Modifier.minimumInteractiveComponentSize().clickable(role = Role.Button) { onDone(items.filterIndexed { j, _ -> j != i }.joinToString("|")) },
                        contentAlignment = Alignment.Center,
                    ) {
                        androidx.compose.material3.Icon(LucideIcons.X, contentDescription = "Remove line ${i + 1}", tint = Palette.TextSecondary, modifier = Modifier.size(18.dp))
                    }
                }
            }
        }
        Row(Modifier.padding(top = 6.dp)) {
            Chip("+ Line", false) { onDone((items + if (items.any { "::" in it }) "::" else "").joinToString("|")) }
        }
    }
}

/** A bare one-line text box. */
@Composable
private fun Box_(value: String, onLeave: () -> Unit, onDone: (String) -> Unit) {
    var text by remember(value) { mutableStateOf(value) }
    BasicTextField(
        text,
        { text = it.replace("|", ""); onDone(text) },
        singleLine = true,
        textStyle = Type.Body.copy(color = Palette.Text, fontSize = 14.sp),
        cursorBrush = SolidColor(Palette.Accent),
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp)).background(Palette.CardPressed).padding(horizontal = 10.dp, vertical = 8.dp)
            .onFocusChanged { if (!it.isFocused) onLeave() },
    )
}

@Composable
private fun Chip(label: String, on: Boolean, choice: Boolean = true, onClick: () -> Unit) {
    Box(
        Modifier.minimumInteractiveComponentSize().clip(RoundedCornerShape(16.dp)).background(if (on) Palette.Accent else Palette.DockBack)
            .then(if (choice) Modifier.selectable(selected = on, role = Role.Button, onClick = onClick) else Modifier.clickable(role = Role.Button, onClick = onClick))
            .padding(horizontal = 12.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, style = Type.Small.copy(color = if (on) Palette.OnAccent else Palette.Text, fontSize = 13.sp), maxLines = 1)
    }
}

/** A one-line text box that hands its words over when they change. */
@Composable
private fun Field(title: String, value: String, hint: String, onLeave: () -> Unit, onDone: (String) -> Unit) {
    var text by remember(value) { mutableStateOf(value) }
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.width(70.dp))
        BasicTextField(
            text,
            { text = it; onDone(it) },
            singleLine = false,
            maxLines = 4,
            textStyle = Type.Body.copy(color = Palette.Text, fontSize = 14.sp),
            cursorBrush = SolidColor(Palette.Accent),
            modifier = Modifier.weight(1f).clip(RoundedCornerShape(6.dp)).background(Palette.CardPressed).padding(horizontal = 10.dp, vertical = 8.dp)
                .onFocusChanged { if (!it.isFocused) onLeave() },
            decorationBox = { inner ->
                Box {
                    if (text.isEmpty()) Text(hint, style = Type.Body.copy(color = Palette.TextSecondary, fontSize = 14.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    inner()
                }
            },
        )
    }
}

/** The settings a drag on the preview moves, by effect: across, and down (either may be absent). */
private val PLACED = mapOf(
    "tap" to ("x" to "y"), "wave-trim" to ("x" to "y"), "swipe" to ("stop" to null), "pause-point" to ("label_x" to null),
    "reject-stamp" to (null to "y"), "messages" to (null to "y"),
)

/** Whether the words box over the preview is open. */
internal object ReelWords {
    var open by mutableStateOf(false)
}

/** The selected item's reel effect, when a reel item (or a clip with the Reel FX panel open) is selected. */
internal fun reelSelected(vm: EditorViewModel): JSONObject? {
    val id = vm.selected ?: return null
    if (vm.panel != Panel.ReelFx && vm.clip(id)?.kind != "layer") return null
    return reelFx(vm, id).firstOrNull()
}

/**
 * The selected reel effect's words, edited over the preview: one line per item (they are kept joined by `|`),
 * opened by a tap on the picture.
 */
@Composable
fun ReelWordsBox(vm: EditorViewModel) {
    if (!ReelWords.open) return
    val id = vm.selected
    val e = reelSelected(vm)
    if (id == null || e == null) { ReelWords.open = false; return }
    val name = e.optString("fx")
    val default = remember(name) { runCatching { reelSettings(name) }.getOrDefault(emptyList()).firstOrNull { it.key == TEXT }?.defaultValue.orEmpty() }
    val now = e.optJSONObject("options")?.optString(TEXT).orEmpty().ifEmpty { default }
    var text by remember(id) { mutableStateOf(now.replace("|", "\n")) }
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(id) { runCatching { focus.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Color(0x99000000)).clickable { ReelWords.open = false }, contentAlignment = Alignment.Center) {
        Column(Modifier.fillMaxWidth().padding(20.dp).clip(RoundedCornerShape(12.dp)).background(Palette.Card).clickable(enabled = false) {}.padding(14.dp)) {
            Text("Words · one line each", style = Type.Small.copy(color = Palette.TextSecondary))
            Spacer(Modifier.height(8.dp))
            BasicTextField(
                text,
                { v ->
                    text = v
                    val list = reelFx(vm, id).map { JSONObject(it.toString()) }
                    list.firstOrNull()?.let { first ->
                        val o = first.optJSONObject("options") ?: JSONObject()
                        val joined = v.split('\n').joinToString("|") { it.replace("|", "") }
                        if (joined.isBlank()) o.remove(TEXT) else o.put(TEXT, joined)
                        first.put("options", o)
                        setReelFx(vm, id, list, "reelfx-words")
                    }
                },
                textStyle = Type.Body.copy(color = Palette.Text, fontSize = 18.sp),
                cursorBrush = SolidColor(Palette.Accent),
                modifier = Modifier.fillMaxWidth().heightIn(min = 60.dp, max = 220.dp).focusRequester(focus),
            )
            Spacer(Modifier.height(10.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                Chip("Done", true) { vm.endGesture(); ReelWords.open = false }
            }
        }
    }
}

/** The selected reel effect's place on the frame (0 to 1 each way), when it is one that can be dragged there. */
internal fun reelHandle(vm: EditorViewModel): Pair<Float, Float>? {
    val id = vm.selected ?: return null
    if (vm.panel != Panel.ReelFx && vm.clip(id)?.kind != "layer") return null
    val e = reelFx(vm, id).firstOrNull() ?: return null
    val (kx, ky) = PLACED[e.optString("fx")] ?: return null
    val o = e.optJSONObject("options")
    fun v(key: String?, d: Float) = key?.let { o?.optString(it)?.toFloatOrNull() } ?: d
    return v(kx, if (kx == "label_x") 0.72f else 0.5f) to v(ky, if (ky == null) 0.5f else 0.35f)
}

/** Puts the selected reel effect at `(x, y)` of the frame, as one gesture. */
internal fun moveReel(vm: EditorViewModel, x: Float, y: Float) {
    val id = vm.selected ?: return
    val list = reelFx(vm, id).map { JSONObject(it.toString()) }
    val e = list.firstOrNull() ?: return
    val (kx, ky) = PLACED[e.optString("fx")] ?: return
    val o = e.optJSONObject("options") ?: JSONObject()
    kx?.let { o.put(it, "%.3f".format(x)) }
    ky?.let { o.put(it, "%.3f".format(y)) }
    e.put("options", o)
    setReelFx(vm, id, list, "reelfx-drag")
}
