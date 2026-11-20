package io.github.walterfr.ultimategamesaver

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // sem enableEdgeToEdge(): a UI web não reserva espaço para a barra de status
    super.onCreate(savedInstanceState)
    askStorageAccess()
  }

  /**
   * O motor em Rust lê os saves com acesso direto a arquivos (std::fs) em /storage/emulated/0.
   * Android 11+: "Acesso a todos os arquivos"; Android 10 ou menor: permissão de armazenamento.
   */
  private fun askStorageAccess() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      if (!Environment.isExternalStorageManager()) {
        startActivity(Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:$packageName")))
      }
    } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
      requestPermissions(arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE, Manifest.permission.WRITE_EXTERNAL_STORAGE), 1)
    }
  }
}
