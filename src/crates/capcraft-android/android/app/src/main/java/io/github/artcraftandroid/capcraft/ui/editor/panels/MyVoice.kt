package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.Manifest
import android.content.pm.PackageManager
import android.media.MediaRecorder
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import io.github.artcraftandroid.capcraft.core.SpeechListener
import io.github.artcraftandroid.capcraft.core.SpeechModel
import io.github.artcraftandroid.capcraft.core.TimelineView
import io.github.artcraftandroid.capcraft.core.captionModels
import io.github.artcraftandroid.capcraft.core.downloadSpeechModel
import io.github.artcraftandroid.capcraft.core.forgetMyVoice
import io.github.artcraftandroid.capcraft.core.hasMyVoice
import io.github.artcraftandroid.capcraft.core.keepMyVoice
import io.github.artcraftandroid.capcraft.core.myVoiceSentence
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The user's own voice, recorded reading a sentence that agrees to its copy. The recording is checked
 * against the sentence, kept on this phone alone and forgotten when asked; no other sound is taken.
 */
@Composable
fun MyVoicePanel(vm: EditorViewModel) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val file = remember { File(context.cacheDir, "my-voice.m4a") }
    var recorder by remember { mutableStateOf<MediaRecorder?>(null) }
    var job by remember { mutableStateOf<String?>(null) }
    var kept by remember { mutableStateOf(hasMyVoice()) }
    var model by remember { mutableStateOf<SpeechModel?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    // a recording made before its model was down, checked when it is
    var waiting by remember { mutableStateOf(false) }

    LaunchedEffect(refresh) {
        model = withContext(Threads.thumbnails) {
            runCatching { captionModels().filter { it.id == "base" || it.id == "base.en" || it.id == "tiny" } }.getOrDefault(emptyList())
                .let { l -> l.firstOrNull { it.downloaded } ?: l.firstOrNull() }
        }
    }
    fun silence() {
        runCatching { recorder?.stop() }
        runCatching { recorder?.release() }
        recorder = null
    }
    DisposableEffect(Unit) {
        onDispose {
            silence()
            file.delete()
        }
    }
    val listener = remember {
        object : SpeechListener {
            override fun onProgress(fraction: Float, stage: String) {
                scope.launch(Dispatchers.Main) { job = (if (stage == "download") "Downloading" else "Checking") + " ${(fraction * 100).toInt()}%" }
            }

            override fun onDone(result: TimelineView?) {
                scope.launch(Dispatchers.Main) {
                    job = null
                    if (waiting) {
                        refresh++
                    } else {
                        kept = hasMyVoice()
                        file.delete()
                        vm.message = "Your voice is ready"
                    }
                }
            }

            override fun onFailed(message: String) {
                scope.launch(Dispatchers.Main) {
                    job = null
                    waiting = false
                    vm.message = message
                }
            }
        }
    }
    fun check() {
        val m = model ?: return
        job = "Checking"
        if (!m.downloaded) {
            waiting = true
            runCatching { downloadSpeechModel(m.id, m.family, listener) }.onFailure { job = null; waiting = false; vm.message = it.userMessage }
            return
        }
        waiting = false
        runCatching { keepMyVoice(file.absolutePath, m.id, listener) }.onFailure { job = null; vm.message = it.userMessage }
    }
    LaunchedEffect(model) { if (waiting && model?.downloaded == true) check() }

    fun start() {
        val r = (if (Build.VERSION.SDK_INT >= 31) MediaRecorder(context) else @Suppress("DEPRECATION") MediaRecorder())
        runCatching {
            r.setAudioSource(MediaRecorder.AudioSource.MIC)
            r.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            r.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
            r.setAudioSamplingRate(44_100)
            r.setAudioEncodingBitRate(128_000)
            r.setOutputFile(file.absolutePath)
            r.prepare()
            r.start()
        }.onSuccess { recorder = r }.onFailure {
            r.release()
            vm.message = "Could not record"
        }
    }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { if (it) start() }

    PanelFrame("My voice", vm, tall = true) {
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 18.dp, vertical = 14.dp)) {
            Text(
                if (kept) "Your voice is on this phone. Text to speech and Text to audio can read in it, with the Pocket TTS model."
                else "Read this out, somewhere quiet, to have your own voice copied. Use your own voice only.",
                style = Type.Small,
            )
            if (!kept) {
                Spacer(Modifier.height(14.dp))
                Text(
                    "“" + myVoiceSentence() + "”",
                    style = Type.Body.copy(fontSize = 17.sp, color = Palette.Text),
                    modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(Color(0xFF333333)).padding(14.dp),
                )
            }
            Spacer(Modifier.height(16.dp))
            val working = job
            when {
                working != null -> WideButton(working, enabled = false) {}
                recorder != null -> WideButton("Stop and check") {
                    silence()
                    check()
                }
                kept -> WideButton("Forget my voice") {
                    runCatching { forgetMyVoice() }.onFailure { vm.message = it.userMessage }
                    kept = hasMyVoice()
                }
                else -> WideButton("Record") {
                    if (ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) start()
                    else ask.launch(Manifest.permission.RECORD_AUDIO)
                }
            }
            Spacer(Modifier.height(10.dp))
            Text("The recording stays on this phone.", style = Type.Tiny.copy(color = Palette.TextSecondary))
            if (kept) {
                Text(
                    "Say it is made by a computer where people could take it for a recording.",
                    style = Type.Tiny.copy(color = Palette.TextSecondary),
                    modifier = Modifier.clickable(enabled = false) {}.padding(top = 4.dp),
                )
            }
        }
    }
}
