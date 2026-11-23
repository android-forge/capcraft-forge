package io.github.artcraftandroid.capcraft.ui.editor.panels

import io.github.artcraftandroid.capcraft.core.ClipView
import io.github.artcraftandroid.capcraft.core.RowKind
import io.github.artcraftandroid.capcraft.core.SpeechListener
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The main-track clip a title or sticker is over, where it starts. */
private fun mainUnder(vm: EditorViewModel, clip: ClipView): ClipView? =
    vm.timeline?.rows?.firstOrNull { it.kind == RowKind.MAIN }?.clips
        ?.firstOrNull { clip.startUs + 1 >= it.startUs && clip.startUs < it.startUs + it.durationUs }

/** Where a clip's row is in the stack, the bottom first. */
private fun stackOf(vm: EditorViewModel, id: String): Int = vm.timeline?.rows?.indexOfFirst { row -> row.clips.any { it.id == id } } ?: -1

/** Whether [clip] is behind the person of the picture under it. */
internal fun isBehindPeople(vm: EditorViewModel, clip: ClipView): Boolean {
    val main = mainUnder(vm, clip) ?: return false
    val copy = bodyOf(vm, main.id)?.first ?: return false
    return stackOf(vm, copy) > stackOf(vm, clip.id)
}

/**
 * A title or sticker put behind the people in the picture under it, as reels set a big word behind
 * someone: the people found and cut out (once), and their copy laid right over this clip, so it shows
 * only where they are not. Again, it comes back in front.
 */
internal fun toggleBehindPeople(vm: EditorViewModel, clip: ClipView) {
    val s = vm.session ?: return
    val main = mainUnder(vm, clip) ?: run { vm.message = "Put it over a clip with people first"; return }
    val scope = MainScope()
    val body = bodyOf(vm, main.id)
    if (body != null && isBehindPeople(vm, clip)) {
        // what was made for this alone goes; a copy made for a light or a look goes back down
        if (body.second == "front") {
            vm.busy = "Bringing it in front"
            scope.launch(Threads.commands) {
                runCatching { s.bodyEffect(main.id, "", listenerFor(vm, scope) {}) }.onFailure { e -> withContext(Dispatchers.Main) { vm.busy = null; vm.message = e.userMessage } }
            }
        } else {
            vm.edit(select = { clip.id }) { placePersonOver(body.first, "") }
        }
        return
    }
    if (body != null) {
        vm.edit(select = { clip.id }) { placePersonOver(body.first, clip.id) }
        return
    }
    vm.busy = "Finding the people"
    scope.launch(Threads.commands) {
        runCatching {
            s.bodyEffect(main.id, "front", listenerFor(vm, scope) {
                val copy = bodyOf(vm, main.id)?.first
                if (copy == null) vm.message = "No people were found there" else vm.edit(select = { clip.id }) { placePersonOver(copy, clip.id) }
            })
        }.onFailure { e -> withContext(Dispatchers.Main) { vm.busy = null; vm.message = e.userMessage } }
    }
}

private fun listenerFor(vm: EditorViewModel, scope: kotlinx.coroutines.CoroutineScope, then: () -> Unit) = object : SpeechListener {
    override fun onProgress(fraction: Float, stage: String) {
        scope.launch { vm.busy = (if (stage == "download") "Downloading the model" else "Finding the people") + " ${(fraction * 100).toInt()}%" }
    }

    override fun onDone(result: TimelineView?) {
        scope.launch {
            vm.busy = null
            result?.let { vm.applyView(it) }
            then()
        }
    }

    override fun onFailed(message: String) {
        scope.launch {
            vm.busy = null
            vm.message = message
        }
    }
}
