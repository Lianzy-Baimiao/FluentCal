import com.android.apksig.ApkVerifier;
import java.io.File;
import java.security.cert.X509Certificate;

/** Checks the signer/installed-data rule used for Android 9+ signing-key rotation.
 * Run with the Android SDK apksigner.jar on the classpath. This does not replace
 * installing the APK on a device and checking that application data survives.
 */
class VerifyAndroidUpgrade {
    private static ApkVerifier.Result verify(String path, int api) throws Exception {
        var result = new ApkVerifier.Builder(new File(path))
            .setMinCheckedPlatformVersion(api).setMaxCheckedPlatformVersion(api).build().verify();
        if (!result.isVerified()) throw new AssertionError(path + " API " + api + ": " + result.getAllErrors());
        return result;
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2 || args.length > 3) throw new IllegalArgumentException("Usage: VerifyAndroidUpgrade OLD.apk NEW.apk [MIN_API=28]");
        int minimum = args.length == 3 ? Integer.parseInt(args[2]) : 28;
        for (int api : new int[] { 24, 27, 28, 32, 33, 36 }) {
            if (api < minimum) continue;
            var old = verify(args[0], api);
            var next = verify(args[1], api);
            if (old.getSignerCertificates().size() != 1 || next.getSignerCertificates().size() != 1)
                throw new AssertionError("Expected one platform signer");
            X509Certificate installed = old.getSignerCertificates().get(0);
            boolean same = installed.equals(next.getSignerCertificates().get(0));
            var lineage = next.getSigningCertificateLineage();
            boolean rotated = lineage != null && lineage.isCertificateInLineage(installed)
                && lineage.getSignerCapabilities(installed).hasInstalledData()
                && !lineage.getSignerCapabilities(installed).hasRollback();
            if (!same && !rotated)
                throw new AssertionError("API " + api + ": incompatible update signature; no installed-data rotation proof");
            System.out.println("PASS API " + api + ": " + (same ? "same signing certificate" : "verified rotation with installed-data capability"));
        }
    }
}
