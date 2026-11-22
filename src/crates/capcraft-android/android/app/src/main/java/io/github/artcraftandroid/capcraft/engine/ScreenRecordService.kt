package io.github.artcraftandroid.capcraft.engine

import android.Manifest
import android.app.Activity
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.MediaRecorder
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.content.IntentCompat
import io.github.artcraftandroid.capcraft.MainActivity
import java.io.File

/**
 * Records the screen once the user has allowed it, with the microphone when they allow that too,
 * until Stop in its notification; what it recorded then opens as a new project.
 */
class ScreenRecordService : Service() {
    private var projection: MediaProjection? = null
    private var display: VirtualDisplay? = null
    private var recorder: MediaRecorder? = null
    private var file: File? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            STOP -> finish()
            START -> if (recorder == null) begin(intent)
        }
        return START_NOT_STICKY
    }

    private fun begin(intent: Intent) {
        val data = IntentCompat.getParcelableExtra(intent, DATA, Intent::class.java)
        val mic = intent.getBooleanExtra(MIC, false) &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
        val started = runCatching {
            // the notification first: the system lets the screen be read only while it is up
            val note = notification()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                var type = ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
                if (mic && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) type = type or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
                startForeground(ID, note, type)
            } else {
                startForeground(ID, note)
            }
            val p = getSystemService(MediaProjectionManager::class.java)
                .getMediaProjection(intent.getIntExtra(CODE, Activity.RESULT_CANCELED), data!!)!!
            projection = p
            p.registerCallback(
                object : MediaProjection.Callback() {
                    // the user ended it from the system's own chip
                    override fun onStop() = finish()
                },
                Handler(Looper.getMainLooper()),
            )
            val metrics = resources.displayMetrics
            // at most 1080 on the short side, both even, as encoders ask
            val scale = minOf(1f, 1080f / minOf(metrics.widthPixels, metrics.heightPixels))
            val bounds = getSystemService(android.view.WindowManager::class.java).let { if (Build.VERSION.SDK_INT >= 30) it.maximumWindowMetrics.bounds else null }
            val w = (((bounds?.width() ?: metrics.widthPixels) * scale).toInt() / 2) * 2
            val h = (((bounds?.height() ?: metrics.heightPixels) * scale).toInt() / 2) * 2
            val out = File(File(cacheDir, "captures").apply { mkdirs() }, "screen-${System.currentTimeMillis()}.mp4")
            file = out
            @Suppress("DEPRECATION")
            val r = if (Build.VERSION.SDK_INT >= 31) MediaRecorder(this) else MediaRecorder()
            recorder = r
            if (mic) r.setAudioSource(MediaRecorder.AudioSource.MIC)
            r.setVideoSource(MediaRecorder.VideoSource.SURFACE)
            r.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            r.setVideoEncoder(MediaRecorder.VideoEncoder.H264)
            if (mic) {
                r.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
                r.setAudioEncodingBitRate(128_000)
                r.setAudioSamplingRate(44_100)
            }
            r.setVideoSize(w, h)
            r.setVideoFrameRate(30)
            r.setVideoEncodingBitRate(12_000_000)
            r.setOutputFile(out.absolutePath)
            r.prepare()
            display = p.createVirtualDisplay("capcraft-screen", w, h, metrics.densityDpi, DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, r.surface, null, null)
            r.start()
        }
        if (started.isFailure) {
            android.util.Log.w("CapCraft", "screen recording failed", started.exceptionOrNull())
            file?.delete()
            file = null
            finish()
        }
    }

    private fun finish() {
        val r = recorder
        val done = file
        recorder = null
        file = null
        // a recording stopped before its first picture throws, and is no recording
        val kept = r != null && runCatching { r.stop() }.isSuccess
        runCatching { r?.release() }
        runCatching { display?.release() }
        display = null
        val p = projection
        projection = null
        runCatching { p?.stop() }
        if (done != null) {
            if (kept && done.length() > 0) ready(done) else done.delete()
        }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        if (recorder != null) finish()
        super.onDestroy()
    }

    private fun channel(): String {
        val manager = getSystemService(NotificationManager::class.java)
        if (manager?.getNotificationChannel(CHANNEL) == null) {
            manager?.createNotificationChannel(NotificationChannel(CHANNEL, "Screen recording", NotificationManager.IMPORTANCE_LOW))
        }
        return CHANNEL
    }

    private fun notification() = NotificationCompat.Builder(this, channel())
        .setSmallIcon(android.R.drawable.presence_video_online)
        .setContentTitle("Recording the screen")
        .setContentText("Tap Stop to finish and edit it")
        .setOngoing(true)
        .addAction(
            0,
            "Stop",
            PendingIntent.getService(this, 1, Intent(this, ScreenRecordService::class.java).setAction(STOP), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
        )
        .build()

    /** Says the recording is there; a tap opens it as a new project. */
    private fun ready(recorded: File) {
        runCatching {
            val uri = FileProvider.getUriForFile(this, "$packageName.files", recorded)
            val open = Intent(this, MainActivity::class.java).setAction(Intent.ACTION_SEND).setType("video/mp4").putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
            val note = NotificationCompat.Builder(this, channel())
                .setSmallIcon(android.R.drawable.presence_video_online)
                .setContentTitle("Screen recorded")
                .setContentText("Tap to edit it")
                .setAutoCancel(true)
                .setContentIntent(PendingIntent.getActivity(this, 2, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
                .build()
            getSystemService(NotificationManager::class.java)?.notify(ID + 1, note)
            pending = uri
        }
    }

    companion object {
        private const val ID = 51
        private const val CHANNEL = "screen"
        private const val START = "start"
        private const val STOP = "stop"
        private const val CODE = "code"
        private const val DATA = "data"
        private const val MIC = "mic"

        /** The last recording made and not opened yet: the app takes it when it is next looked at. */
        @Volatile var pending: android.net.Uri? = null

        /** Starts recording with what the system's consent screen answered. */
        fun start(context: Context, code: Int, data: Intent, mic: Boolean) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, ScreenRecordService::class.java).setAction(START).putExtra(CODE, code).putExtra(DATA, data).putExtra(MIC, mic),
            )
        }
    }
}
