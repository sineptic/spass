{
  rustPlatform,
  pkgs,
}:
rustPlatform.buildRustPackage {
  pname = "spass";
  version = "0.2.0";

  src = ./.;
  cargoHash = "sha256-RSPURlmfL+0JaSQrPeAjTXM9TrBB8J69Z7aBnPyjGME=";
  nativeBuildInputs = with pkgs; [
    pkg-config
  ];
  buildInputs = with pkgs; [
    libgpg-error
    gpgme
  ];
}
