package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.github.artcraftandroid.capcraft.core.PeopleLight
import io.github.artcraftandroid.capcraft.core.SpeechListener
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** The copy of [clipId]'s people a body effect made, by id, with the effect's name; null when it has none. */
internal fun bodyOf(vm: EditorViewModel, clipId: String): Pair<String, String>? = runCatching {
    val all = vm.session?.extra("capcraftBody")?.takeIf { it.isNotBlank() }?.let(::JSONObject) ?: return null
    val mine = all.optJSONObject(clipId) ?: return null
    mine.optString("copy") to mine.optString("kind")
}.getOrNull()?.takeIf { vm.clip(it.first) != null }

/**
 * Light people: looks at how the people in the clip are lit and gives them the light they want - more
 * where they are dark or against the light, less where they are burnt out - the rest of the picture
 * left as it is. Well-lit people are left alone, whatever the colour of their skin. How much is a slider.
 */
@Composable
fun LightPeoplePanel(vm: EditorViewModel) {
    val clip = vm.selectedClip ?: run { vm.panel = null; return }
    val scope = rememberCoroutineScope()
    var light by remember(clip.id) { mutableStateOf<PeopleLight?>(null) }
    var job by remember { mutableStateOf<String?>(null) }
    var said by remember { mutableStateOf<String?>(null) }
    var strength by remember(clip.id) { mutableFloatStateOf(100f) }
    val on = bodyOf(vm, clip.id)?.second?.startsWith("light:") == true

    fun kind(l: PeopleLight) = "light:%.3f:%.1f:%.1f:%.3f:%.3f:%.3f:%.2f".format(java.util.Locale.US, l.exposure, l.shadows, l.highlights, l.faceX, l.faceY, l.faceSize, l.faceStops)

    val listener = remember {
        object : SpeechListener {
            override fun onProgress(fraction: Float, stage: String) {
                scope.launch(Dispatchers.Main) { job = (if (stage == "download") "Downloading the model" else "Finding the people") + " ${(fraction * 100).roundToInt()}%" }
            }

            override fun onDone(result: TimelineView?) {
                scope.launch(Dispatchers.Main) {
                    job = null
                    result?.let { vm.applyView(it, clip.id) }
                }
            }

            override fun onFailed(message: String) {
                scope.launch(Dispatchers.Main) {
                    job = null
                    vm.message = message
                }
            }
        }
    }
    fun put(l: PeopleLight) {
        val s = vm.session ?: return
        job = "Lighting"
        scope.launch(Threads.commands) {
            runCatching { s.bodyEffect(clip.id, kind(l), listener) }.onFailure { e -> withContext(Dispatchers.Main) { job = null; vm.message = e.userMessage } }
        }
    }
    fun look() {
        val s = vm.session ?: return
        job = "Looking at the light"
        said = null
        val at = vm.playhead
        scope.launch {
            val found = withContext(Threads.thumbnails) { runCatching { s.peopleLight(clip.id, at) } }
            job = null
            found.onSuccess { l ->
                light = l
                when {
                    l.people <= 0f -> said = "No people were found here."
                    l.exposure == 0f && l.shadows == 0f && l.highlights == 0f && l.faceStops == 0f -> said = "The people are well lit already."
                    else -> {
                        said = if (l.exposure < 0) "They were too bright. Eased." else if (l.faceStops > 0) "Their face was in shadow. Lit." else if (l.shadows > 0) "They were against the light. Lit." else "They were too dark. Lit."
                        strength = 100f
                        put(l)
                    }
                }
            }.onFailure { vm.message = it.userMessage }
        }
    }
    // opened on a clip with no light of this kind, it looks at once
    LaunchedEffect(clip.id) { if (!on) look() }

    /** The light at [percent] of what was worked out, set on the people's copy. */
    fun scale(percent: Float) {
        val l = light ?: return
        val (copy, _) = bodyOf(vm, clip.id) ?: return
        val chain = vm.clipJson(copy)?.optJSONArray("videoEffects") ?: JSONArray()
        val next = JSONArray()
        val last = (0 until chain.length()).lastOrNull { chain.getJSONObject(it).optString("id") == "concat.adjust" } ?: return
        for (i in 0 until chain.length()) {
            val link = JSONObject(chain.getJSONObject(i).toString())
            val k = percent / 100.0
            if (i == last) {
                link.put("params", (link.optJSONObject("params") ?: JSONObject()).put("exposure", l.exposure * k).put("shadows", l.shadows * k).put("highlights", l.highlights * k))
            }
            if (link.optString("id") == "capcraft.face-light") {
                link.put("params", (link.optJSONObject("params") ?: JSONObject()).put("stops", l.faceStops * k))
            }
            next.put(link)
        }
        vm.patch(copy, JSONObject().put("videoEffects", next), "light-people")
    }

    PanelFrame("Light people", vm, height = 240.dp, left = {
        if (on && job == null) {
            Text(
                "Take off",
                style = Type.Small.copy(color = Palette.DockText),
                modifier = Modifier.clickable {
                    said = null
                    val s = vm.session ?: return@clickable
                    scope.launch(Threads.commands) { runCatching { s.bodyEffect(clip.id, "", listener) } }
                }.padding(8.dp),
            )
        }
    }) {
        Text(
            job ?: said ?: if (on) "The people here have been lit." else "Gives people the light they want, and leaves the rest of the picture alone.",
            style = Type.Small.copy(color = Palette.Text),
            modifier = Modifier.padding(horizontal = 18.dp, vertical = 16.dp),
        )
        if (on && light != null && job == null) {
            ValueSlider(
                value = strength,
                range = 0f..150f,
                label = { "${it.roundToInt()}" },
                onChange = { strength = it; scale(it) },
                onDone = { vm.endGesture() },
                title = "Strength",
            )
        }
        Spacer(Modifier.weight(1f))
        Box(Modifier.padding(horizontal = 16.dp, vertical = 10.dp)) { WideButton(if (on) "Look again here" else "Light people", job == null) { look() } }
        Spacer(Modifier.height(2.dp))
    }
}
