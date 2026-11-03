// Кнопка шелла: фаска, жёсткая тень, при нажатии вдавливается на пиксель
import QtQuick

Item {
    id: btn
    property string text: ""
    property string icon: ""
    property var pal
    property bool primary: false
    property bool ghost: false
    signal clicked()
    width: Math.max(40, row.width + 28)
    height: 32
    activeFocusOnTab: true

    readonly property bool hov: ma.containsMouse || activeFocus
    readonly property bool down: ma.pressed

    PxBox {
        x: btn.down ? 1 : 0; y: btn.down ? 1 : 0
        width: btn.width; height: btn.height
        visible: !btn.ghost || btn.hov
        fill: btn.primary ? (btn.hov ? pal.bright : pal.accent) : (btn.hov ? pal.surfaceHi : (btn.ghost ? "transparent" : pal.surface))
        outline: btn.primary ? "transparent" : (btn.hov ? pal.accent : pal.border)
        shadow: btn.ghost || btn.down ? 0 : 2
        bevel: !btn.ghost
    }
    Row {
        id: row
        x: (btn.width - width) / 2 + (btn.down ? 1 : 0)
        y: (btn.height - height) / 2 + (btn.down ? 1 : 0)
        spacing: 7
        PxIcon { visible: btn.icon !== ""; name: btn.icon; cell: 2; color: btn.primary ? pal.onAccent : (btn.hov ? "white" : pal.accent); anchors.verticalCenter: parent.verticalCenter }
        PxText { visible: btn.text !== ""; text: btn.text; px: 16; color: btn.primary ? pal.onAccent : (btn.hov ? "white" : pal.text); anchors.verticalCenter: parent.verticalCenter }
    }
    MouseArea { id: ma; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: btn.clicked() }
    Keys.onReturnPressed: btn.clicked()
    Keys.onSpacePressed: btn.clicked()
}
