package sh.zeron.android.ui

import android.net.Uri
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.BottomAppBar
import androidx.compose.material3.BottomAppBarDefaults
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import kotlinx.coroutines.launch
import sh.zeron.android.core.AppModel
import sh.zeron.android.design.ZIcon
import sh.zeron.android.design.ZIcons
import sh.zeron.android.design.ZeronTheme

@Composable
fun ZeronRoot(model: AppModel) {
    val appearance by model.appearance.collectAsState()
    ZeronTheme(appearance) {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            val client by model.client.collectAsState()
            Box(Modifier.fillMaxSize()) {
                AnimatedContent(client != null, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "root") { signedIn ->
                    if (signedIn) MainNav(model) else SignInScreen(model)
                }
                // Mic prompt, keep-awake and the off-stage "call ended" snackbar.
                sh.zeron.android.voice.VoiceHost(model.voice)
            }
        }
    }
}

object Routes {
    const val HOME = "home"
    const val CHAT = "chat/{id}"
    const val NEW = "new"
    const val SEARCH = "search"
    const val SETTINGS = "settings"
    const val ARCHIVED = "archived"
    const val VOICE = "voice"
    const val VOICE_SETTINGS = "voice-settings"
    const val TERMINAL = "terminal/{chat}?id={id}"
    const val SUBAGENT = "subagent/{parent}/{doc}?title={title}&running={running}"
    fun chat(id: String) = "chat/$id"
    fun terminal(chat: String, id: String?) = "terminal/${Uri.encode(chat)}" + (id?.let { "?id=${Uri.encode(it)}" } ?: "")
    fun subagent(parent: String, doc: String, title: String, running: Boolean) =
        "subagent/${Uri.encode(parent)}/${Uri.encode(doc)}?title=${Uri.encode(title)}&running=$running"
}

@Composable
private fun MainNav(model: AppModel) {
    val nav = rememberNavController()
    // Large screens (tablets, unfolded foldables): sessions beside the open chat.
    androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxSize()) {
    // (Width alone would split a phone turned sideways.)
    val wide = maxWidth >= 840.dp && maxHeight >= 480.dp
    var selected by rememberSaveable { mutableStateOf<String?>(null) }
    val open: (String) -> Unit = { id -> if (wide) selected = id else nav.navigate(Routes.chat(id)) }
    // Folding back to a phone keeps the open chat, full screen.
    LaunchedEffect(wide) {
        val id = selected
        if (!wide && id != null) {
            selected = null
            nav.navigate(Routes.chat(id))
        }
    }
    LaunchedEffect(Unit) {
        when (val route = model.launch.route) {
            null -> Unit
            "new" -> nav.navigate(Routes.NEW)
            "search" -> nav.navigate(Routes.SEARCH)
            "settings", "more" -> nav.navigate(Routes.SETTINGS)
            "archived" -> nav.navigate(Routes.ARCHIVED)
            else -> if (route.startsWith("chat:")) nav.navigate(Routes.chat(route.removePrefix("chat:")))
        }
    }
    // The notification, a snackbar Retry or `voice-stage` asks for the call.
    LaunchedEffect(Unit) {
        model.voice.stageRequests.collect {
            if (nav.currentDestination?.route != Routes.VOICE) nav.navigate(Routes.VOICE) { launchSingleTop = true }
        }
    }
    // A debug `companion` extra opens over the launch route's chat, once.
    var companionPending by rememberSaveable { mutableStateOf(model.launch.companion) }

    /** A session page with its companions wired to the nav stack. */
    @Composable
    fun Session(id: String, onBack: () -> Unit) {
        val launchChat = model.launch.route?.removePrefix("chat:")
        SessionScreen(
            model,
            id,
            onBack = onBack,
            onOpenVoice = { nav.navigate(Routes.VOICE) { launchSingleTop = true } },
            onVoiceSettings = { nav.navigate(Routes.VOICE_SETTINGS) },
            onOpenSideChat = { nav.navigate(Routes.chat(it)) },
            onOpenTerminal = { chat, terminal -> nav.navigate(Routes.terminal(chat, terminal)) },
            onOpenSubagent = { parent, doc, title, running -> nav.navigate(Routes.subagent(parent, doc, title, running)) },
            openCompanion = companionPending?.takeIf { id == launchChat }?.also { companionPending = null },
        )
    }

    Box(
        Modifier.fillMaxSize().onPreviewKeyEvent { e ->
            // Hardware keyboards: Ctrl+N new session, Ctrl+Shift+F search (iPad ⌘N / ⌘⇧F).
            if (e.type != KeyEventType.KeyDown || !(e.isCtrlPressed || e.isMetaPressed)) return@onPreviewKeyEvent false
            when {
                e.key == Key.N -> nav.navigate(Routes.NEW).let { true }
                e.key == Key.F && e.isShiftPressed -> nav.navigate(Routes.SEARCH).let { true }
                else -> false
            }
        },
    ) {
    NavHost(nav, startDestination = Routes.HOME) {
        composable(Routes.HOME) {
            if (!wide) {
                Home(model, nav, open)
            } else {
                Row(Modifier.fillMaxSize()) {
                    Box(Modifier.width(400.dp).fillMaxHeight()) { Home(model, nav, open, selected) }
                    androidx.compose.material3.VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    Box(Modifier.weight(1f).fillMaxHeight()) {
                        val id = selected
                        if (id != null) {
                            key(id) { Session(id, onBack = { selected = null }) }
                        } else {
                            Column(Modifier.align(Alignment.Center), horizontalAlignment = Alignment.CenterHorizontally) {
                                ZIcon(ZIcons.Chat, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.outline)
                                androidx.compose.foundation.layout.Spacer(Modifier.size(12.dp))
                                Text("Pick a session, or start a new one", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        }
        composable(Routes.ARCHIVED) {
            ArchivedScreen(model, onBack = { nav.popBackStack() }, onOpen = { nav.navigate(Routes.chat(it)) })
        }
        composable(Routes.SETTINGS) {
            Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                SettingsScreen(
                    model,
                    onArchived = { nav.navigate(Routes.ARCHIVED) },
                    onOpenVoiceSettings = { nav.navigate(Routes.VOICE_SETTINGS) },
                    onBack = { nav.popBackStack() },
                )
            }
        }
        composable(Routes.CHAT) { entry ->
            val id = entry.arguments?.getString("id") ?: return@composable
            Session(id, onBack = { nav.popBackStack() })
        }
        composable(
            Routes.TERMINAL,
            arguments = listOf(navArgument("chat") { type = NavType.StringType }, navArgument("id") { type = NavType.StringType; nullable = true }),
        ) { entry ->
            val chat = entry.arguments?.getString("chat") ?: return@composable
            TerminalScreen(
                model,
                chat,
                terminalId = entry.arguments?.getString("id"),
                onBack = { nav.popBackStack() },
                script = if (model.launch.terminalDemo) listOf("git log --oneline -5", "cargo test -p zeron-veil") else emptyList(),
            )
        }
        composable(
            Routes.SUBAGENT,
            arguments = listOf(
                navArgument("parent") { type = NavType.StringType },
                navArgument("doc") { type = NavType.StringType },
                navArgument("title") { type = NavType.StringType; defaultValue = "Subagent" },
                navArgument("running") { type = NavType.BoolType; defaultValue = false },
            ),
        ) { entry ->
            val args = entry.arguments ?: return@composable
            SubagentScreen(
                model,
                parentId = args.getString("parent") ?: return@composable,
                docId = args.getString("doc") ?: return@composable,
                title = args.getString("title") ?: "Subagent",
                running = args.getBoolean("running"),
                onBack = { nav.popBackStack() },
                onOpenSubagent = { parent, doc, title, running -> nav.navigate(Routes.subagent(parent, doc, title, running)) },
            )
        }
        composable(Routes.VOICE) {
            sh.zeron.android.voice.VoiceStageScreen(
                model.voice,
                onClose = { nav.popBackStack() },
                onOpenChat = { chatId ->
                    nav.popBackStack()
                    open(chatId)
                },
                wallpaper = model.wallpaper,
            )
        }
        composable(Routes.VOICE_SETTINGS) {
            sh.zeron.android.voice.VoiceSettingsScreen(model.voice, onBack = { nav.popBackStack() })
        }
        composable(Routes.NEW) {
            NewSessionScreen(model, onClose = { nav.popBackStack() }, onCreated = { id ->
                nav.popBackStack()
                open(id)
            })
        }
        composable(Routes.SEARCH) {
            SearchScreen(model, onBack = { nav.popBackStack() }, onOpen = { id ->
                if (wide) nav.popBackStack()
                open(id)
            })
        }
    }
    Toasts(model, nav)
    }
    }
}

/** App-wide snackbars ("Archived · Undo"), just above whatever bottom chrome the page has. */
@Composable
private fun androidx.compose.foundation.layout.BoxScope.Toasts(model: AppModel, nav: NavHostController) {
    val host = remember { SnackbarHostState() }
    LaunchedEffect(Unit) {
        model.toasts.collect { toast ->
            // One at a time: a new toast replaces the one showing.
            host.currentSnackbarData?.dismiss()
            launch {
                val result = host.showSnackbar(toast.message, actionLabel = toast.action, withDismissAction = toast.action != null, duration = SnackbarDuration.Long)
                if (result == SnackbarResult.ActionPerformed) toast.run?.invoke()
            }
        }
    }
    val entry by nav.currentBackStackEntryAsState()
    val inset = when (entry?.destination?.route) {
        Routes.HOME -> 96.dp
        Routes.CHAT, Routes.NEW -> 96.dp
        Routes.TERMINAL, Routes.SUBAGENT -> 72.dp
        else -> 16.dp
    }
    SnackbarHost(host, Modifier.align(Alignment.BottomCenter).imePadding().navigationBarsPadding().padding(bottom = inset))
}

/**
 * The front page: the Sessions list over a Material 3 bottom app bar —
 * Settings, Search and voice as its actions, New session as its FAB (iOS's
 * bottom bar: Settings · Search · voice · New session). The bar slides away
 * while the list scrolls down and comes back on the way up.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
private fun Home(model: AppModel, nav: NavHostController, open: (String) -> Unit, selected: String? = null) {
    val bar = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        Box(Modifier.fillMaxSize().nestedScroll(bar.nestedScrollConnection)) {
            SessionsScreen(model, onOpen = open, onArchived = { nav.navigate(Routes.ARCHIVED) }, selected = selected)
        }
        val voice = sh.zeron.android.voice.rememberVoiceVisible(model.voice)
        BottomAppBar(
            actions = {
                Tip("Settings") {
                    IconButton(onClick = { nav.navigate(Routes.SETTINGS) }) { ZIcon(ZIcons.Settings, "Settings", Modifier.size(24.dp)) }
                }
                Tip("Search") {
                    IconButton(onClick = { nav.navigate(Routes.SEARCH) }) { ZIcon(ZIcons.Search, "Search", Modifier.size(24.dp)) }
                }
                if (voice) {
                    // Tap calls (or opens the live call); hold picks the host and style.
                    sh.zeron.android.voice.VoiceHeaderButton(
                        model.voice,
                        onOpenStage = { nav.navigate(Routes.VOICE) { launchSingleTop = true } },
                        onOpenSettings = { nav.navigate(Routes.VOICE_SETTINGS) },
                        size = 48.dp,
                        container = Color.Transparent,
                    )
                }
            },
            floatingActionButton = {
                Tip("New session") {
                    FloatingActionButton(
                        onClick = { nav.navigate(Routes.NEW) },
                        containerColor = MaterialTheme.colorScheme.primary,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                        elevation = androidx.compose.material3.FloatingActionButtonDefaults.bottomAppBarFabElevation(),
                    ) { ZIcon(ZIcons.NewSession, "New session", Modifier.size(24.dp)) }
                }
            },
            containerColor = MaterialTheme.colorScheme.surfaceContainer,
            scrollBehavior = bar,
            modifier = Modifier.align(Alignment.BottomCenter),
        )
    }
}

/** A long-press label for an icon-only control. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
private fun Tip(text: String, content: @Composable () -> Unit) {
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text(text) } },
        state = rememberTooltipState(),
        content = content,
    )
}
