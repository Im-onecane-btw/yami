// yami — экран входа SDDM в стиле экрана блокировки шелла: обои под тёмной вуалью со
// строками развёртки, большие пиксельные часы, дата, поле пароля с фаской. Неверный пароль —
// поле вздрагивает ступенями. Внизу — сеанс, раскладка, питание; в углу 闇.
// Появление и уход — клетками по Байеру (dissolve.frag), как панели шелла.
// Цвета — из theme.conf / theme.conf.user (их пишет tools/theme.py по палитре шелла).
import QtQuick

Rectangle {
    id: root
    width: 1920
    height: 1080
    color: pal.bg

    function cfg(k, d) {
        var v = config[k]
        return (v === undefined || v === null || ("" + v) === "") ? d : ("" + v)
    }
    readonly property var pal: ({
        accent: cfg("accent", "#c4161c"),
        deep: cfg("deep", "#7a0e11"),
        bright: cfg("bright", "#ff1d24"),
        bg: cfg("bg", "#0a0707"),
        panel: cfg("panel", "#100c0c"),
        surface: cfg("surface", "#191212"),
        surfaceHi: cfg("surfaceHi", "#231b1b"),
        codeBg: cfg("codeBg", "#060404"),
        border: cfg("border", "#4b1112"),
        text: cfg("text", "#ece4e4"),
        textDim: cfg("textDim", "#8e8181"),
        textFaint: cfg("textFaint", "#5f5151"),
        onAccent: cfg("onAccent", "#ffffff"),
        warn: cfg("warn", "#ffaa28")
    })
    readonly property bool en: {
        var l = cfg("lang", "")
        if (l === "en") return true
        if (l === "ru") return false
        return Qt.locale().name.indexOf("ru") !== 0
    }
    function tr(ru, en) { return root.en ? en : ru }

    FontLoader { id: tiny; source: "fonts/Tiny5.ttf" }
    FontLoader { id: jp; source: "fonts/DotGothic16-Regular.ttf" }

    property int userIdx: Math.max(0, userModel.lastIndex !== undefined ? userModel.lastIndex : 0)
    property int sessionIdx: Math.max(0, sessionModel.lastIndex !== undefined ? sessionModel.lastIndex : 0)
    property int fails: 0
    property bool busy: false
    property string status: ""
    property real reveal: 0
    property int layoutIdx: keyboard ? keyboard.currentLayout : 0

    // имена из моделей SDDM
    Repeater { id: users; model: userModel; delegate: Item { property string uname: model.name || ""; property string rname: model.realName || "" } }
    Repeater { id: sessions; model: sessionModel; delegate: Item { property string sname: model.name || "" } }
    readonly property string userName: users.count > 0 && users.itemAt(userIdx) ? users.itemAt(userIdx).uname : (userModel.lastUser || "")
    readonly property string sessionName: sessions.count > 0 && sessions.itemAt(sessionIdx) ? sessions.itemAt(sessionIdx).sname : ""

    function login() {
        if (busy || pass.text.length === 0) return
        busy = true
        status = tr("проверяю…", "checking…")
        sddm.login(userName, pass.text, sessionIdx)
    }

    Connections {
        target: sddm
        function onLoginFailed() {
            busy = false
            fails += 1
            status = ""
            pass.text = ""
            shake.restart()
        }
        function onLoginSucceeded() {
            status = ""
            out.start()
        }
    }

    NumberAnimation on reveal { id: inAnim; from: 0; to: 1; duration: 420; easing.type: Easing.OutCubic; running: true }
    NumberAnimation { id: out; target: root; property: "reveal"; to: 0; duration: 260; easing.type: Easing.InQuad }

    Item {
        id: content
        anchors.fill: parent

        Image {
            id: wall
            anchors.fill: parent
            source: Qt.resolvedUrl(cfg("background", "background.png"))
            fillMode: Image.PreserveAspectCrop
            smooth: cfg("pixelate", "true") !== "true"
            asynchronous: false
        }
        Rectangle { anchors.fill: parent; color: "#8c000000" }
        // строки развёртки — через одну, еле видно
        Repeater {
            model: Math.ceil(root.height / 4)
            Rectangle { y: index * 4; width: root.width; height: 1; color: "#2e000000" }
        }

        // ── часы и дата ──
        property date now: new Date()
        Timer { interval: 1000; running: true; repeat: true; onTriggered: content.now = new Date() }
        readonly property string clock: Qt.formatTime(now, "HH:mm")
        readonly property int cy: root.height / 2 - 150

        PxText { x: (root.width - width) / 2 + 4; y: content.cy + 4; px: 128; text: content.clock; color: pal.deep; opacity: 0.8 }
        PxText { id: clockText; x: (root.width - width) / 2; y: content.cy; px: 128; text: content.clock; color: pal.text }
        PxText {
            id: dateText
            x: (root.width - width) / 2
            y: clockText.y + clockText.height + 6
            px: 24
            color: pal.textDim
            text: {
                var s = Qt.locale(root.en ? "en_US" : "ru_RU").toString(content.now, "dddd, d MMMM")
                return root.en ? s : s.toLowerCase()
            }
        }

        // ── вход ──
        Item {
            id: box
            width: 380
            height: 140
            x: (root.width - width) / 2
            y: dateText.y + dateText.height + 70 - 30

            Row {
                y: 0
                spacing: 8
                PxIcon { name: "lock"; cell: 2; color: pal.accent; anchors.verticalCenter: parent.verticalCenter }
                PxText { text: root.userName; px: 16; color: pal.text; anchors.verticalCenter: parent.verticalCenter }
            }
            // выбор пользователя, если их несколько
            Row {
                visible: users.count > 1
                anchors.right: parent.right
                y: -4
                spacing: 2
                PxButton { pal: root.pal; ghost: true; icon: "left"; width: 26; height: 24; onClicked: root.userIdx = (root.userIdx + users.count - 1) % users.count }
                PxButton { pal: root.pal; ghost: true; icon: "right"; width: 26; height: 24; onClicked: root.userIdx = (root.userIdx + 1) % users.count }
            }

            Item {
                id: fieldWrap
                y: 30
                width: parent.width
                height: 36
                property int dx: 0
                PxBox { x: -6 + fieldWrap.dx; y: -6; width: parent.width + 12; height: parent.height + 12; fill: Qt.rgba(0, 0, 0, 0.35); outline: pal.border; shadow: 4 }
                PxBox { id: field; x: fieldWrap.dx; width: parent.width; height: parent.height; fill: pal.codeBg; outline: pass.activeFocus ? pal.accent : pal.border }
                // точки пароля — квадраты 8×8, курсор — уголь
                Item {
                    x: field.x + 10
                    width: field.width - 20
                    height: field.height
                    clip: true
                    Row {
                        id: dots
                        anchors.verticalCenter: parent.verticalCenter
                        x: Math.min(0, parent.width - width - 14)
                        spacing: 4
                        Repeater { model: pass.text.length; Rectangle { width: 8; height: 8; color: pal.text } }
                    }
                    PxText {
                        visible: pass.text.length === 0
                        anchors.verticalCenter: parent.verticalCenter
                        x: 12
                        px: 16
                        text: root.tr("пароль", "password")
                        color: pal.textFaint
                    }
                    Rectangle {
                        id: caret
                        x: pass.text.length === 0 ? 0 : dots.x + dots.width + 4
                        anchors.verticalCenter: parent.verticalCenter
                        width: 8; height: 16
                        color: pal.bright
                        visible: pass.activeFocus && blink.on
                    }
                }
                Timer { id: blink; property bool on: true; interval: 530; running: true; repeat: true; onTriggered: on = !on }
                TextInput {
                    id: pass
                    anchors.fill: parent
                    opacity: 0
                    echoMode: TextInput.Password
                    focus: true
                    enabled: !root.busy
                    onTextChanged: { blink.on = true; if (root.fails > 0 && text.length > 0) root.status = "" }
                    Keys.onReturnPressed: root.login()
                    Keys.onEnterPressed: root.login()
                    Keys.onEscapePressed: text = ""
                    Keys.onUpPressed: if (users.count > 1) root.userIdx = (root.userIdx + users.count - 1) % users.count
                    Keys.onDownPressed: if (users.count > 1) root.userIdx = (root.userIdx + 1) % users.count
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onClicked: pass.forceActiveFocus() }
            }
            SequentialAnimation {
                id: shake
                PropertyAction { target: fieldWrap; property: "dx"; value: 8 }
                PauseAnimation { duration: 60 }
                PropertyAction { target: fieldWrap; property: "dx"; value: -8 }
                PauseAnimation { duration: 60 }
                PropertyAction { target: fieldWrap; property: "dx"; value: 6 }
                PauseAnimation { duration: 60 }
                PropertyAction { target: fieldWrap; property: "dx"; value: -6 }
                PauseAnimation { duration: 60 }
                PropertyAction { target: fieldWrap; property: "dx"; value: 4 }
                PauseAnimation { duration: 60 }
                PropertyAction { target: fieldWrap; property: "dx"; value: -4 }
                PauseAnimation { duration: 60 }
                PropertyAction { target: fieldWrap; property: "dx"; value: 0 }
            }
            PxText {
                id: statusText
                x: (parent.width - width) / 2
                y: fieldWrap.y + fieldWrap.height + 12
                px: 16
                readonly property bool caps: keyboard && keyboard.capsLock
                text: root.status !== "" ? root.status : (caps ? "caps lock" : (root.fails > 0 ? root.tr("неверно ×", "wrong ×") + root.fails : root.tr("enter — войти", "enter — log in")))
                color: caps || (root.fails > 0 && root.status === "") ? (caps ? pal.warn : pal.bright) : pal.textFaint
            }
        }

        // ── низ: сеанс, раскладка, питание ──
        Row {
            x: 24
            y: root.height - 56
            spacing: 8
            PxButton {
                pal: root.pal
                icon: "grid"
                text: root.sessionName !== "" ? root.sessionName : root.tr("сеанс", "session")
                visible: sessions.count > 0
                onClicked: root.sessionIdx = (root.sessionIdx + 1) % Math.max(1, sessions.count)
            }
            PxButton {
                pal: root.pal
                icon: "kb"
                visible: keyboard && keyboard.layouts && keyboard.layouts.length > 1
                text: keyboard && keyboard.layouts && keyboard.layouts[root.layoutIdx] ? keyboard.layouts[root.layoutIdx].shortName : ""
                onClicked: keyboard.currentLayout = (keyboard.currentLayout + 1) % keyboard.layouts.length
            }
        }
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            y: root.height - 56
            spacing: 8
            PxButton { pal: root.pal; ghost: true; icon: "moon"; text: root.tr("сон", "sleep"); visible: sddm.canSuspend; onClicked: sddm.suspend() }
            PxButton { pal: root.pal; ghost: true; icon: "reboot"; text: root.tr("перезагрузка", "reboot"); visible: sddm.canReboot; onClicked: sddm.reboot() }
            PxButton { pal: root.pal; ghost: true; icon: "power"; text: root.tr("выключить", "power off"); visible: sddm.canPowerOff; onClicked: sddm.powerOff() }
        }
        Text {
            visible: cfg("kanji", "true") === "true"
            x: root.width - 72
            y: root.height - 64
            text: "闇"
            font.family: jp.name
            font.pixelSize: 32
            color: pal.accent
            opacity: 0.5
            renderType: Text.NativeRendering
        }
        PxText {
            x: 24
            y: 20
            px: 16
            text: sddm.hostName || ""
            color: pal.textFaint
        }
    }

    // проявление клетками по Байеру
    ShaderEffectSource { id: src; sourceItem: content; hideSource: root.reveal < 1; live: true; visible: false }
    ShaderEffect {
        anchors.fill: parent
        visible: root.reveal < 1
        property variant source: src
        property real k: root.reveal
        property real cell: 2
        property size size: Qt.size(width, height)
        fragmentShader: "dissolve.frag.qsb"
    }

    Component.onCompleted: pass.forceActiveFocus()
}
