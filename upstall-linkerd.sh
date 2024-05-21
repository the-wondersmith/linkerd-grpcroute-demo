#!/usr/bin/env bash

HERE="$(pwd)"

declare -i reset=0 build=0 emojivoto=0 arg_count=${#}
 
for (( idx=0; idx<=arg_count; idx++ )); do
  case ${!idx} in
    "-b" | "--build")
        build=1
        ;;
    "-e" | "--emojivoto")
        emojivoto=1
        ;;
    "-r" | "--remove")
        reset=1
        ;;
    "-x" | "--reset")
        build=0 reset=1 emojivoto=1
        break
        ;;
    "${0}")
        continue
        ;;
    *)
        echo -e "Bad arg: ${!idx}\n\nUsage: ${0} [-b|--build] [-e|--emojivoto] [-r|--remove] [-x|--reset]\n" && exit 127
        ;;
  esac
done


function error() {
    cd "${HERE}"
    exit ${1}
}

function remove-linkerd() {
    bin/linkerd install --ignore-cluster | kubectl delete --ignore-not-found=true -f -
    bin/linkerd install --crds --ignore-cluster 2> /dev/null | kubectl delete --ignore-not-found=true -f -
}


function remove-emojivoto() {
    kubectl delete --ignore-not-found=true -f https://run.linkerd.io/emojivoto.yml || error 12
}

function upgrade-linkerd() {
    echo -e "\n🔧 upgrading linkerd crds ...\n"
    bin/linkerd upgrade --crds | kubectl apply -f - || error 5
    echo -e "\n🔨 upgrading linkerd control plane ...\n"
    bin/linkerd upgrade --set proxyInit.runAsRoot=true --set='policyController.logLevel=linkerd=debug\,info' | kubectl apply -f - || error 6
    echo -e "\n🕙 waiting for new linkerd control plane to report ready ...\n"
    kubectl -n linkerd rollout status deployments --watch=true --timeout=300s --selector='app.kubernetes.io/part-of=Linkerd' || error 7
    echo -e "\n🧹 cleaning up orphaned linkerd resources ...\n"
    bin/linkerd prune | kubectl delete -f - || error 8
    echo -e "✨ linkerd control plane upgrade complete!\n"
}

function install-or-upgrade-linkerd() {
    if kubectl -n linkerd get configmap/linkerd-config 2> /dev/null 1>&2; then
        upgrade-linkerd
    elif ! kubectl get namespace/linkerd 2> /dev/null 1>&2 && ! bin/linkerd check --pre; then
        echo "linkerd precheck failed" && error 1
    elif kubectl get namespace/anus-party 2> /dev/null 1>&2 && bin/linkerd check; then
        upgrade-linkerd || error 2
    else
        echo -e "\n📋 installing linkerd crds ...\n"
        bin/linkerd install --crds | kubectl apply -f - || error 2
        echo -e "\n🏗️  installing linkerd control plane ...\n"
        bin/linkerd install --set proxyInit.runAsRoot=true --set='policyController.logLevel=linkerd=debug\,info' | kubectl apply -f - || error 3
        echo -e "\n🕙 waiting for linkerd control plane to report ready ...\n"
        kubectl -n linkerd rollout status deployments --watch=true --timeout=300s --selector='app.kubernetes.io/part-of=Linkerd' || error 4
        echo -e "\n🎉 linkerd control plane installation complete!\n"
    fi
}

function install-or-upgrade-emojivoto() {
    if ! kubectl get pods --selector='app.kubernetes.io/part-of=emojivoto' -n emojivoto 2>&1 > /dev/null; then
        echo -e "\n🏗️  deploying emojivoto app ...\n"
        waiting="🕙 waiting for all emojivoto components to report ready ..."
        upgraded="🎉 emojivoto app deployed!"
    else
        echo -e "\n🛠️  upgrading deployed emojivoto instance ...\n"
        waiting="🕙 waiting for new emojivoto components to report ready ..."
        upgraded="✨ emojivoto instance upgraded!"
    fi

    bin/linkerd inject https://run.linkerd.io/emojivoto.yml | \
    yq -ePo yaml '
      select( . != null )
      | .metadata.labels += {"app.kubernetes.io/part-of": "emojivoto"}
      | select( .kind == "Deployment" ) as $deployment
      | select( .kind != "Deployment" ) as $other
      | $deployment.spec.template.metadata.labels += {"app.kubernetes.io/part-of": "emojivoto"}
      | ($deployment, $other)
    ' | kubectl apply -f - || error 9
    echo -e "\n${waiting}\n"
    kubectl -n emojivoto rollout status deployments --watch=true --timeout=300s --selector='app.kubernetes.io/part-of=emojivoto' || error 10
    echo -e "\n${upgraded}\n"
}


cd "${LINKERD_SRC:-${HOME}/workspace/linkerd2}"

if [[ $build -eq 1 ]]; then
    bin/docker-build || error 99
fi

if [[ $reset -eq 1 ]]; then
    if [[ $emojivoto -eq 1 ]]; then
        remove-emojivoto
    fi
    remove-linkerd
    error 0
fi

install-or-upgrade-linkerd

if [[ $emojivoto -eq 1 ]]; then
    install-or-upgrade-emojivoto
fi

cd "${HERE}"
