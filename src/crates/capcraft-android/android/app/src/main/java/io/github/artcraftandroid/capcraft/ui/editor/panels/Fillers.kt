package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.core.Filler
import io.github.artcraftandroid.capcraft.core.FillerListener
import io.github.artcraftandroid.capcraft.core.SpeechListener
import io.github.artcraftandroid.capcraft.core.SpeechModel
import io.github.artcraftandroid.capcraft.core.TimeRange
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.core.cancelSpeech
import io.github.artcraftandroid.capcraft.core.captionModels
import io.github.artcraftandroid.capcraft.core.downloadSpeechModel
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.clock
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Glyph
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Filler words: the "um"s and "uh"s of the talk, listed with their times to hear and choose from,
 * then cut out of the main track together. The sounds that mean nothing are chosen to start with;
 * phrases that may be meant ("you know") wait to be. Heard on the phone, as the captions are.
 */
@Composable
fun FillersPanel(vm: EditorViewModel) {
    val scope = rememberCoroutineScope()
    var model by remember { mutableStateOf<SpeechModel?>(null) }
    var job by remember { mutableStateOf<Pair<String, Float>?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    // set by a finished download, so listening follows on its own
    var then by remember { mutableStateOf(false) }
    var found by remember { mutableStateOf<List<Filler>?>(null) }
    var off by remember { mutableStateOf(setOf<Int>()) }

    LaunchedEffect(refresh) {
        model = withContext(Threads.thumbnails) {
            // the larger model hears hesitations the smaller one skips
            runCatching { captionModels().filter { it.id == "base" || it.id == "base.en" } }.getOrDefault(emptyList())
                .let { list -> list.firstOrNull { it.downloaded } ?: list.firstOrNull() }
        }
    }
    DisposableEffect(Unit) { onDispose { if (job != null) runCatching { cancelSpeech() } } }

    fun stageName(stage: String) = when (stage) {
        "download" -> "Downloading"
        "unpack" -> "Unpacking"
        "read" -> "Reading the sound"
        "load" -> "Loading"
        else -> "Listening"
    }
    val listener = remember {
        object : FillerListener {
            override fun onProgress(fraction: Float, stage: String) {
                scope.launch(Dispatchers.Main) { job = stageName(stage) to fraction }
            }

            override fun onDone(fillers: List<Filler>) {
                scope.launch(Dispatchers.Main) {
                    job = null
                    found = fillers
                    off = fillers.indices.filter { !fillers[it].sound }.toSet()
                }
            }

            override fun onFailed(message: String) {
                scope.launch(Dispatchers.Main) { job = null; vm.message = message }
            }
        }
    }
    val download = remember {
        object : SpeechListener {
            override fun onProgress(fraction: Float, stage: String) {
                scope.launch(Dispatchers.Main) { job = stageName(stage) to fraction }
            }

            override fun onDone(result: TimelineView?) {
                scope.launch(Dispatchers.Main) { job = null; then = true; refresh++ }
            }

            override fun onFailed(message: String) {
                scope.launch(Dispatchers.Main) { job = null; vm.message = message }
            }
        }
    }

    fun listen() {
        val m = model ?: return
        if (!m.downloaded) {
            job = "Downloading" to 0f
            runCatching { downloadSpeechModel(m.id, m.family, download) }.onFailure { job = null; vm.message = it.userMessage }
            return
        }
        val s = vm.session ?: return
        job = "Reading the sound" to 0f
        scope.launch(Threads.commands) {
            runCatching { s.findFillers(m.id, listener) }.onFailure { e -> launch(Dispatchers.Main) { job = null; vm.message = e.userMessage } }
        }
    }
    LaunchedEffect(model) {
        if (then && model?.downloaded == true) {
            then = false
            listen()
        }
    }

    fun remove() {
        val all = found ?: return
        val ranges = all.filterIndexed { i, _ -> i !in off }.map { TimeRange(it.startUs, it.endUs) }
        if (ranges.isEmpty()) return
        vm.edit {
            val done = cutMany(ranges, vm.transcript.orEmpty())
            scope.launch(Dispatchers.Main) {
                if (vm.transcript != null) vm.transcript = done.lines
                vm.message = if (ranges.size == 1) "1 filler removed" else "${ranges.size} fillers removed"
                vm.panel = null
            }
            done.view
        }
    }

    PanelFrame("Filler words", vm, tall = true, titleOnTop = true) {
        val running = job
        val list = found
        when {
            running != null -> Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(30.dp))
                Text("${running.first} ${(running.second * 100).toInt()}%", style = Type.BodyStrong)
                Spacer(Modifier.height(12.dp))
                Box(Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(2.dp)).background(Color(0xFF3A3A3A))) {
                    Box(Modifier.fillMaxWidth(running.second.coerceIn(0f, 1f)).height(4.dp).background(Palette.Accent))
                }
                Spacer(Modifier.height(16.dp))
                Text(
                    "Stop",
                    style = Type.BodyStrong.copy(color = Palette.DockText),
                    modifier = Modifier.clickable { runCatching { cancelSpeech() }; job = null }.padding(8.dp),
                )
            }
            list == null -> Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(24.dp))
                Text(
                    "Find \"um\", \"uh\" and other fillers in what is said, then cut the ones you choose.",
                    style = Type.Body.copy(color = Palette.Text, textAlign = TextAlign.Center),
                )
                Spacer(Modifier.height(22.dp))
                val m = model
                WideButton(
                    if (m != null && !m.downloaded) "Download and listen (%.0f MB)".format(m.sizeBytes.toLong() / 1_048_576.0) else "Find filler words",
                    m != null,
                ) { listen() }
                Spacer(Modifier.height(8.dp))
                Text("Made on this phone; nothing is uploaded.", style = Type.Tiny.copy(color = Palette.TextSecondary))
            }
            list.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("No filler words were found", style = Type.Body.copy(color = Palette.TextSecondary))
            }
            else -> Column(Modifier.fillMaxSize()) {
                LazyColumn(Modifier.fillMaxWidth().weight(1f).padding(horizontal = 8.dp)) {
                    itemsIndexed(list) { i, filler ->
                        val on = i !in off
                        Row(
                            Modifier.fillMaxWidth().height(50.dp).clip(RoundedCornerShape(8.dp))
                                .clickable { vm.scrub((filler.startUs - 400_000).coerceAtLeast(0)); vm.start() }
                                .padding(start = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(clock(filler.startUs), style = Type.Small.copy(fontSize = 12.sp, color = Palette.TextSecondary), modifier = Modifier.width(52.dp))
                            Text(filler.text, style = Type.Body.copy(fontSize = 15.sp, color = if (on) Palette.Text else Palette.TextSecondary), modifier = Modifier.weight(1f))
                            Box(
                                Modifier.size(44.dp).clickable { off = if (on) off + i else off - i },
                                contentAlignment = Alignment.Center,
                            ) {
                                Box(
                                    Modifier.size(20.dp).clip(CircleShape)
                                        .then(if (on) Modifier.background(Palette.Accent) else Modifier.border(1.5.dp, Palette.TextSecondary, CircleShape)),
                                    contentAlignment = Alignment.Center,
                                ) { if (on) Glyph(LucideIcons.Check, Color.Black, size = 13.dp, description = "Chosen") }
                            }
                        }
                    }
                }
                val count = list.size - off.size
                Box(Modifier.padding(horizontal = 16.dp, vertical = 10.dp)) {
                    WideButton(if (count == 0) "Choose what to remove" else "Remove ($count)", count > 0) { remove() }
                }
            }
        }
    }
}

/** A panel's one wide button. */
@Composable
internal fun WideButton(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        Modifier.fillMaxWidth().height(46.dp).clip(RoundedCornerShape(8.dp)).background(if (enabled) Palette.Accent else Color(0xFF3A3A3A)).clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Text(label, style = Type.BodyStrong.copy(fontSize = 16.sp, color = if (enabled) Palette.OnAccent else Palette.TextSecondary)) }
}
