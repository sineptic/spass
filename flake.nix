{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
  };

  outputs = {
    self,
    nixpkgs,
    ...
  }: let
    pkgs = nixpkgs.legacyPackages.x86_64-linux;
  in {
    packages.x86_64-linux.spass = pkgs.callPackage ./build.nix {};
    packages.x86_64-linux.default = self.packages.x86_64-linux.spass;
  };
}
