# Local Dev Environment

TL;DR: Rootless Docker, Minikube and Tilt.

Run, **from the root** of the repository:
```shell
tilt up -f dev/Tiltfile
```

# Setting up a local development environment

For this development environment, we will use:
- Rootless Docker
- Minikube
- Tilt

See the references at the end for extra documentation and information.

> [!NOTE]
> The commands bellow perform a few changes to your system.
> If you don't feel comfortable running them as is, check the official
> documentation and follow the steps there instead.

## Rootless Docker

We assume you already have Docker installed. If not, do it now.

1. Disable regular docker, cleanup any old images and containers first:
```shell
sudo docker stop $(docker ps -a -q)
sudo docker rm $(docker ps -a -q)
sudo docker system prune -a
sudo systemctl disable --now docker docker.socket
```

2. Install rootless docker.

On Ubuntu and similar, you can install the docker-ce-rootless-extras package:
```shell
sudo apt-get install -y docker-ce-rootless-extras
```

On Arch and similar, install the docker-rootless-extras package from the AUR:
```shell
sudo yay -Syu docker-rootless-extras
```

3. Make sure your user can impersonate 65536 UIDs and GIDs:

The numbers below may need to be adjusted if other users in this system already
have this setup:
```shell
sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 "$USER"
```

4. Allow unprivileged user namespace cloning:

```shell
sudo sh -c "echo 'kernel.unprivileged_userns_clone=1' > /etc/sysctl.d/80-rootless-docker.conf"
sudo sysctl --system
```

5. Make systemd delegate cpu and cpuset to users (only io memory pids are
enabled by default):

```shell
sudo mkdir -p /etc/systemd/system/user@.service.d/
sudo cat > /etc/systemd/system/user@.service.d/delegate.conf <<EOF
[Service]
Delegate=cpu cpuset io memory pids
EOF
sudo systemctl daemon-reload
```

6. Enable rootless docker by default:

```shell
systemctl --user enable --now docker.socket
```

7. Create a rootless docker context and use it:

```
docker context create rootless --docker "host=unix://$XDG_RUNTIME_DIR/docker.sock"
docker context use rootless
```

## Optional build performance for Arch systems

Enable native overlay diff engine to speed up image builds:

```
sudo sh -c "echo 'options overlay metacopy=off redirect_dir=off' > /etc/modprobe.d/disable-overlay-redirect-dir.conf"
```

## Minikube

Install minikube, as normal.

As a workaroud for a [networking issue](https://github.com/kubernetes/minikube/issues/16962),
allow ipv4 forwarding. This step may not be necessary if the issue is fixed:

```shell
sudo sh -c "echo 'net.ipv4.ip_forward=1' > /etc/sysctl.d/85-minikube-ipv4-fix.conf"
```

**Reboot!**

After finishing this setup, the author still had a few networking issues that
were solved by rebooting, even after reloading parametrs with `sysctl`.

Then, launch minikube specifying the container runtime "containerd":

```
minikube start --driver=docker --container-runtime=containerd
```


Tilt requires a local registry, fortunately we can use Minikube's addon:
```sh
minikube addons enable registry
```

This exposes a local registry in `localhost:32770` in your host, and
`localhost:5000` from inside minikube itself.

## Tilt

Install it normally, and then run, **from the root of this repository**:
```
tilt up -f dev/Tiltfile
```

A local instance of pod-director will be available in the pod-director
namespace. Check the [values-tilt.yaml](./values-tilt.yaml) file for
extra configurations for testing and validation.
