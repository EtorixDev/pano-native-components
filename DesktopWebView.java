package dev.etorix.panoscrobbler;

// all params should be non null
// this class is just useed for testing

public class DesktopWebView {
    private static native void startEventLoop();

    static native void launchWebView(String url, String callbackPrefix, String cookiesUrl, String dataDir, String proxyHost, int proxyPort);

    static native void deleteAndQuit();

    static {
        System.loadLibrary("native_webview");
    }

    public static void main(String[] args) {
        launchWebView(
                "https://fonts.google.com",
                "callbackPrefix",
                "https://fonts.google.com",
                "/tmp/webview",
                "",
                0
        );
        
        new Thread(new Runnable() {
            @Override
            public void run() {
                startEventLoop();
            }
        }).start();

        try {
            Thread.sleep(5000);
        } catch (InterruptedException e) {
            e.printStackTrace();
        }

        deleteAndQuit();
    }

    public static void onCallback(String url, String[] cookies) {
        System.out.println("onCallback: " + url);
        for (String cookie : cookies) {
            System.out.println("Cookie: " + cookie);
        }
    }

}
