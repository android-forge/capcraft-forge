package io.github.artcraftandroid.capcraft.engine

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import io.github.artcraftandroid.capcraft.MainActivity
import io.github.artcraftandroid.capcraft.R

/** The Home screen widget: a button each to start a video, a photo, the camera or a screen recording. */
class StartWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        val views = RemoteViews(context.packageName, R.layout.widget_start)
        listOf(R.id.widget_new to "new", R.id.widget_photo to "photo", R.id.widget_shoot to "shoot", R.id.widget_record to "record").forEachIndexed { i, (view, start) ->
            views.setOnClickPendingIntent(
                view,
                PendingIntent.getActivity(context, 100 + i, opening(context, start), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
            )
        }
        ids.forEach { manager.updateAppWidget(it, views) }
    }

    companion object {
        /** What the app is opened to start: "new", "photo", "shoot" or "record". */
        const val START = "capcraft.start"

        private fun opening(context: Context, start: String) = Intent(context, MainActivity::class.java)
            .setAction("capcraft.START.$start")
            .putExtra(START, start)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
    }
}
