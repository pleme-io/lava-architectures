(defcaixa
  :name
  "lava-architectures"
  :kind
  :Biblioteca
  :ecosystem
  :rust-single-crate
  :package
  {:name "lava-architectures"
   :version "0.1.0"
   :description "Reusable infrastructure compositions for the lava suite. Hand-authored typed architectures (AwsVpcNetwork, AkeylessAwsIntegration, EksScaleTest, etc.). Pangea-architectures analog — port produces byte-equivalent terraform.json so state files match between pangea+tofu and lava+magma."
   :license "MIT"
   :repository "https://github.com/pleme-io/lava-architectures"}
  :ci-config
  {:bump {:default-type "patch"}
   :publish {:no-verify true}}
  :workflows
  [:auto-release :pre-merge-gate :security-gate])
