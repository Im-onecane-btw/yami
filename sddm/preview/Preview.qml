// Предпросмотр темы без SDDM: подставные sddm, userModel, sessionModel, config, keyboard.
// tools/sddm-preview.sh рисует его в PNG (offscreen) или открывает окном.
import QtQuick
import QtQuick.Window

Window {
    id: win
    width: 1280
    height: 720
    visible: true
    color: "black"
    property string out: Qt.application.arguments.length > 0 ? Qt.application.arguments[Qt.application.arguments.length - 1] : ""

    QtObject {
        id: sddm
        property bool canPowerOff: true
        property bool canReboot: true
        property bool canSuspend: true
        property string hostName: "archlinux"
        signal loginFailed()
        signal loginSucceeded()
        function login(u, p, s) { loginFailed() }
        function powerOff() {}
        function reboot() {}
        function suspend() {}
    }
    ListModel {
        id: userModel
        property int lastIndex: 0
        property string lastUser: "trendorin"
        ListElement { name: "trendorin"; realName: "Trendorin" }
        ListElement { name: "guest"; realName: "Guest" }
    }
    ListModel {
        id: sessionModel
        property int lastIndex: 0
        ListElement { name: "niri" }
        ListElement { name: "Plasma (Wayland)" }
    }
    QtObject {
        id: keyboard
        property bool capsLock: false
        property int currentLayout: 0
        property var layouts: [{ shortName: "us", longName: "English" }, { shortName: "ru", longName: "Russian" }]
    }
    property var config: ({})

    Loader {
        id: loader
        anchors.fill: parent
        source: "THEME/Main.qml"
    }
    // снимок: дождаться проявления и сохранить
    Timer {
        interval: 900
        running: win.out.slice(-4) === ".png"
        onTriggered: win.contentItem.grabToImage(function(r) { r.saveToFile(win.out); Qt.quit() })
    }
}
